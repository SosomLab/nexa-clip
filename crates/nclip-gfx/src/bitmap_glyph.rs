//! 비트맵 글리프(컬러 이모지) 디코드 + 캐시 — T-18f.
//!
//! Noto Color Emoji(CBDT/CBLC)·Apple Color Emoji(sbix)는 윤곽 없이 **글리프마다 PNG**를 담는다.
//! [`crate::text`]가 윤곽이 없는 글리프에서 이 모듈을 불러 PNG를 RGBA로 풀고, 그 결과를 재사용한다.
//!
//! # 🔴 디코더 경계 (사용자 결정 10-05 · T-18f · 시스템 글꼴 한정 예외)
//!
//! 프로젝트 규칙은 "이미지 디코더는 격리 워커 `nclip-imgdec`에만 둔다"이다 — 클립보드로 들어온
//! 이미지는 **남이 만든 바이트**라서 본체 프로세스에서 풀지 않는다. 이 모듈은 그 규칙의 **유일한 예외**다:
//!
//! - 여기서 푸는 PNG는 **시스템 글꼴 파일 안의 글리프 비트맵뿐**이다(신뢰하는 로컬 파일 ·
//!   `nclip-plat::font`가 고정 경로에서 mmap한 바이트 → `ab_glyph`가 표에서 잘라 준 조각).
//! - **클립보드 이미지·원격 수신 이미지·그 밖의 외부 바이트를 이 함수로 보내면 안 된다** —
//!   그것들은 계속 격리 워커를 거친다. 그래서 이 모듈은 `pub(crate)`이고 진입점이
//!   `ab_glyph::v2::GlyphImage` 하나뿐이다(임의 바이트를 받는 공개 API를 만들지 않는다).
//! - 예외 안에서도 상한을 둔다: PNG 512 KB · 512×512 px 초과와 PNG가 아닌 형식은 풀지 않는다
//!   (그 글리프는 종전처럼 빈칸).

use crate::surface::IconImage;
use ab_glyph::v2::GlyphImage;
use ab_glyph::GlyphImageFormat;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// 글리프 PNG 한 장의 바이트 상한(Noto Color Emoji 실측은 장당 수 KB).
const PNG_BYTES_MAX: usize = 512 * 1024;
/// 글리프 비트맵 한 변 상한(px) — Noto 136×128 · Apple 최대 160.
const SIDE_MAX: u32 = 512;
/// 캐시 항목 상한 — 넘으면 **통째로 비운다**(가장 단순한 축출 · 화면에 보이는 것은 다음 그리기에 다시 푼다).
/// 136×128 RGBA ≈ 70 KB → 최악 약 9 MB. 이모지를 안 쓰면 0.
const CACHE_MAX: usize = 128;

/// 캐시 키 = (폰트 바이트 주소, TTC 인덱스, 글리프 id).
type Key = (usize, u32, u16);
/// 값 = (푼 스트라이크의 ppem, 비트맵). 비트맵 `None` = 풀 수 없음(같은 스트라이크는 다시 시도하지 않는다).
type Cache = HashMap<Key, (u16, Option<Arc<IconImage>>)>;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// (본, 글리프)의 디코드된 비트맵 — 처음 한 번만 풀고 이후는 캐시에서 준다.
///
/// 글리프당 한 장만 둔다. 스트라이크가 여럿인 글꼴(sbix)에서 글자 크기가 바뀌어 다른 스트라이크가
/// 골라지면(`raw.pixels_per_em`이 다름) 그 장으로 교체한다 — 호출 측은 항상 `raw`의 ppem으로 배율을 낸다.
pub(crate) fn cached(
    face: (usize, u32),
    glyph: u16,
    raw: &GlyphImage<'_>,
) -> Option<Arc<IconImage>> {
    let key = (face.0, face.1, glyph);
    let mut map = cache().lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((ppem, hit)) = map.get(&key) {
        if *ppem == raw.pixels_per_em {
            return hit.clone();
        }
    }
    let decoded = decode(raw).map(Arc::new);
    if map.len() >= CACHE_MAX && !map.contains_key(&key) {
        map.clear();
    }
    map.insert(key, (raw.pixels_per_em, decoded.clone()));
    decoded
}

/// 글리프 PNG → straight-alpha RGBA. 상한 밖·PNG 아님·깨진 데이터 = `None`.
fn decode(raw: &GlyphImage<'_>) -> Option<IconImage> {
    if !matches!(raw.format, GlyphImageFormat::Png) || raw.data.len() > PNG_BYTES_MAX {
        return None;
    }
    let mut decoder = png::Decoder::new(raw.data);
    // 팔레트 → RGB(A) · 저비트 그레이 확장 · 16 → 8비트(nclip-imgdec와 같은 정규화).
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    // 할당 전에 자른다 — PNG 헤더가 주장하는 크기 기준.
    if info.width == 0 || info.height == 0 || info.width > SIDE_MAX || info.height > SIDE_MAX {
        return None;
    }
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let out = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (out.width, out.height);
    let channels = match out.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return None, // EXPAND 뒤에는 나오지 않는다.
    };
    let row_len = w as usize * channels;
    let data = buf.get(..out.buffer_size())?;
    if out.line_size < row_len || data.len() < out.line_size * h as usize {
        return None;
    }
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for row in data.chunks(out.line_size).take(h as usize) {
        for px in row[..row_len].chunks(channels) {
            match *px {
                [r, g, b, a] => rgba.extend_from_slice(&[r, g, b, a]),
                [r, g, b] => rgba.extend_from_slice(&[r, g, b, 255]),
                [v, a] => rgba.extend_from_slice(&[v, v, v, a]),
                [v] => rgba.extend_from_slice(&[v, v, v, 255]),
                _ => return None,
            }
        }
    }
    (rgba.len() == w as usize * h as usize * 4).then(|| IconImage::from_rgba(w, h, rgba))
}
