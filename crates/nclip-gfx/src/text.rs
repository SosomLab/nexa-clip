//! 텍스트 스택 최소 경로 — **ab_glyph**(SP-1c 실측 후 사용자 확정 08-08).
//!
//! 폰트 파싱(ttf-parser 계열)·글리프 래스터만 쓴다. **셰이핑 엔진은 v1에 없다** — 한글은
//! 완성형 음절이 cmap에 직접 있어 글리프 치환이 필요 없고(아랍어·인도계와 다른 점), v1 요구는
//! 한/영(FR-U-3)이다. 복잡 문자는 v2에서 이 모듈 뒤(DR-21 이음새)에 셰이핑을 추가한다.
//!
//! **폰트 바이트는 밖에서 온다** — 이 크레이트는 파일을 읽지 않는다(플랫폼 중립).
//! 시스템 폰트 경로 발견은 `nclip-plat` 소관(ADR-0001 — 폰트 열거는 플랫폼 계층).

use crate::bitmap_glyph;
use crate::surface::{Color, Surface};
use ab_glyph::{Font as _, FontRef, GlyphId, ScaleFont as _};

/// 폴백 체인의 한 본 — 파싱된 얼굴 + **비트맵 글리프 캐시용 신원**(T-18f).
#[derive(Clone)]
struct Face {
    font: FontRef<'static>,
    /// (폰트 바이트 주소, TTC 인덱스) — 바이트가 `'static`이라 프로세스 안에서 유일하다.
    key: (usize, u32),
}

impl Face {
    fn parse(data: &'static [u8], index: u32) -> Result<Self, FontError> {
        FontRef::try_from_slice_and_index(data, index)
            .map(|font| Self {
                font,
                key: (data.as_ptr() as usize, index),
            })
            .map_err(|_| FontError)
    }
}

impl core::ops::Deref for Face {
    type Target = FontRef<'static>;
    fn deref(&self) -> &FontRef<'static> {
        &self.font
    }
}

/// 로드된 폰트 — **프로세스 수명 자원**(로드 1회 · 앱 종료까지 사용).
///
/// 바이트는 `&'static`이다 — `nclip-plat`의 mmap(파일 백드 페이지 · 힙 0)이 정상 경로이고,
/// [`Font::from_bytes`]는 소유 바이트를 의도적으로 누수해 같은 표현으로 수렴한다(테스트·특수 경로용).
pub struct Font {
    /// ★ 폴백 체인(09-01 사용자 요청 "두부 예방") — [0] = 주 폰트, 이후 = 대체.
    /// 글자마다 글리프가 있는 첫 보을 쓴다(JetBrains Mono + 한글 = 시스템 본이 받는다).
    faces: Vec<Face>,
}

impl Clone for Font {
    fn clone(&self) -> Self {
        Self {
            faces: self.faces.clone(),
        }
    }
}

impl core::fmt::Debug for Font {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Font").finish_non_exhaustive()
    }
}

/// 폰트 로드 실패(파싱 불가·인덱스 없음).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontError;

/// 텍스트 스타일(faux 볼드·이탤릭).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TextStyle {
    /// 굵게(faux — 이중 그리기).
    pub bold: bool,
    /// 기울임(faux — 전단).
    pub italic: bool,
}

impl TextStyle {
    /// 스타일 없음.
    pub const PLAIN: Self = Self {
        bold: false,
        italic: false,
    };
}

impl Font {
    /// `'static` 폰트 바이트에서 로드한다(mmap 정상 경로). `index`는 TTC 컬렉션 인덱스.
    ///
    /// # Errors
    /// 파싱 불가·인덱스 범위 밖이면 [`FontError`].
    pub fn from_static(data: &'static [u8], index: u32) -> Result<Self, FontError> {
        Face::parse(data, index).map(|f| Self { faces: vec![f] })
    }

    /// ★ 폴백 폰트 추가(09-01) — 주 폰트에 없는 글자만 이 보이 받는다.
    /// 기준선(ascent·줄 높이)은 주 폰트가 계속 정한다 — 줌 안 주 글꼴이 섮여도 행이 안 흔들린다.
    ///
    /// # Errors
    /// 파싱 불가·인덱스 범위 밖이면 [`FontError`].
    pub fn push_fallback(&mut self, data: &'static [u8], index: u32) -> Result<(), FontError> {
        self.faces.push(Face::parse(data, index)?);
        Ok(())
    }

    /// 글자가 있는 첫 보 — 없으면 주 폰트(.notdef 표시가 정직하다).
    /// ★ 다른 글꼴의 얼굴 전부를 폴백으로 잇는다(09-04 — 고정폭 글꼴에 주 글꼴 체인을 통째로).
    pub fn push_fallback_font(&mut self, other: &Font) {
        self.faces.extend(other.faces.iter().cloned());
    }

    fn face_for(&self, ch: char) -> &Face {
        self.faces
            .iter()
            .find(|f| f.glyph_id(ch).0 != 0)
            .unwrap_or(&self.faces[0])
    }

    /// 소유 바이트에서 로드 — **의도적 누수**로 `'static`화(폰트는 프로세스 수명 자원).
    ///
    /// # Errors
    /// 파싱 불가·인덱스 범위 밖이면 [`FontError`].
    pub fn from_bytes(data: Vec<u8>, index: u32) -> Result<Self, FontError> {
        Self::from_static(Box::leak(data.into_boxed_slice()), index)
    }

    /// 이 폰트가 문자의 글리프를 갖고 있는가(폴백 체인 판단 근거).
    #[must_use]
    pub fn covers(&self, ch: char) -> bool {
        self.faces.iter().any(|f| f.glyph_id(ch).0 != 0)
    }

    /// `size`(px)에서의 줄 높이.
    #[must_use]
    pub fn line_height(&self, size: f32) -> f32 {
        let s = self.faces[0].as_scaled(size);
        s.ascent() - s.descent() + s.line_gap()
    }

    /// 텍스트 폭(px) — 그리지 않고 잰다(라벨 실측 정렬 — [docs/12 §B]).
    #[must_use]
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                self.control_advance(c, size).unwrap_or_else(|| {
                    let face = self.face_for(c);
                    face.as_scaled(size).h_advance(face.glyph_id(c))
                })
            })
            .sum()
    }

    /// ★ 제어 문자 표시 규칙(09-03 실기 — 탭이 두부(□)로 그려졌다):
    /// 탭 = **공백 4칸 폭**(글리프는 그리지 않음) · 그 외 제어(CR 등) = 폭 0.
    /// 측정과 그리기가 같은 규칙을 쓰므로 캐럿 좌표도 일관된다.
    fn control_advance(&self, c: char, size: f32) -> Option<f32> {
        if c == '\t' {
            let face = self.face_for(' ');
            return Some(face.as_scaled(size).h_advance(face.glyph_id(' ')) * 4.0);
        }
        c.is_control().then_some(0.0)
    }

    /// `size`에서의 어센트(베이스라인 위 높이, px) — 상단 기준 배치를 베이스라인으로 변환.
    #[must_use]
    pub fn ascent(&self, size: f32) -> f32 {
        self.faces[0].as_scaled(size).ascent()
    }

    /// 이 폰트에 `c`의 글리프가 있는가(.notdef = 없음) — 슬롯 폴백 판단용(08-10).
    #[must_use]
    pub fn has_glyph(&self, c: char) -> bool {
        self.covers(c)
    }

    /// `size`에서 숫자 '0'의 **실측 외곽 높이**(px) — 광학 크기 보정용(08-10).
    /// 같은 px라도 폰트마다 숫자가 차지하는 높이가 달라(Consolas ≫ 맑은 고딕)
    /// 나란히 그리면 커 보인다. 외곽선이 없으면 경험 근사(0.7em).
    #[must_use]
    pub fn digit_height(&self, size: f32) -> f32 {
        let g = self.faces[0]
            .glyph_id('0')
            .with_scale_and_position(size, ab_glyph::point(0.0, 0.0));
        self.faces[0]
            .outline_glyph(g)
            .map_or(size * 0.7, |og| og.px_bounds().height())
    }

    /// `size`에서의 텍스트 상자 높이(어센트+디센트, px) — 세로 중앙 정렬 실측용.
    /// `line_height`와 달리 줄 간격(line gap)을 빼서 한 줄 배치에 쓴다.
    #[must_use]
    pub fn text_box_height(&self, size: f32) -> f32 {
        let s = self.faces[0].as_scaled(size);
        s.ascent() - s.descent()
    }

    /// ★ 비트맵 글리프(CBDT·sbix의 PNG) 한 개를 `size`에 맞춰 축소·확대해 그린다(T-18f).
    ///
    /// 윤곽이 없는 글리프에서만 불린다. 비트맵도 없으면(공백 등) 아무것도 하지 않는다 — 종전 동작 그대로.
    /// 배치: 스트라이크 ppem → 요청 ppem 비율 `k`로, 왼쪽 = 펜 + `origin.x·k`,
    /// 아래 = 베이스라인 − `origin.y·k`(글꼴 좌표는 y가 위로 — ttf-parser가 CBDT·sbix 모두 "아래 변"으로 맞춰 준다).
    /// 전진 폭은 호출 측이 hmtx로 옮긴다(측정과 동일).
    #[allow(clippy::too_many_arguments)]
    fn draw_bitmap_glyph(
        &self,
        surface: &mut Surface<'_>,
        face: &Face,
        gid: GlyphId,
        pen: f32,
        baseline: f32,
        size: f32,
        clip: (i32, i32, i32, i32),
    ) {
        // ab_glyph의 `size`는 (ascent − descent) 높이다 → em당 픽셀로 환산해 스트라이크를 고른다.
        let height = face.height_unscaled();
        let Some(upem) = face.units_per_em() else {
            return;
        };
        if height <= 0.0 || size <= 0.0 {
            return;
        }
        let ppem = size * upem / height;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let want = ppem.ceil().clamp(1.0, f32::from(u16::MAX)) as u16;
        // 표 조회만(디코드 없음) — 비트맵 표가 없는 본은 여기서 바로 끝난다(캐시도 건드리지 않는다).
        let Some(raw) = face.glyph_raster_image2(gid, want) else {
            return;
        };
        if raw.pixels_per_em == 0 {
            return;
        }
        let Some(img) = bitmap_glyph::cached(face.key, gid.0, &raw) else {
            return;
        };
        let k = ppem / f32::from(raw.pixels_per_em);
        #[allow(clippy::cast_precision_loss)]
        let (dw, dh) = (img.w as f32 * k, img.h as f32 * k);
        let left = pen + raw.origin.x * k;
        let bottom = baseline - raw.origin.y * k;
        #[allow(clippy::cast_possible_truncation)]
        surface.blend_image_scaled(
            left.round() as i32,
            (bottom - dh).round() as i32,
            (dw.round() as i32).max(1),
            (dh.round() as i32).max(1),
            &img,
            clip,
        );
    }

    /// [`Font::draw_text`]의 클립 변형 — `clip = (x0, y0, x1, y1)` 밖 픽셀은 찍지 않는다
    /// (행 배경 안에서만 그리는 `text_opaque` 모델의 기초).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_clipped(
        &self,
        surface: &mut Surface<'_>,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        text: &str,
        clip: (i32, i32, i32, i32),
    ) -> f32 {
        self.draw_styled(surface, x, y, size, color, text, clip, TextStyle::PLAIN)
    }

    /// [`Font::draw_text_clipped`]의 **스타일 변형** — 실제 볼드/이탤릭 폰트 파일 없이
    /// **faux 볼드**(x축 2회 그리기)·**faux 이탤릭**(베이스라인 위 거리 비례 전단)로 근사한다.
    /// 진짜 글꼴 패밀리·굵기 face는 폰트 열거(M3-3 확장)에서. 지금은 시스템 폰트 1벌 위 근사.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_styled(
        &self,
        surface: &mut Surface<'_>,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        text: &str,
        clip: (i32, i32, i32, i32),
        style: TextStyle,
    ) -> f32 {
        let slant = if style.italic { 0.22 } else { 0.0 };
        let bold_pass = if style.bold { 2 } else { 1 };
        let mut pen = x;
        for ch in text.chars() {
            // ★ 제어 문자 = 폭만 옮기고 글리프 없음(탭 두부 차단 · 09-03).
            if let Some(adv) = self.control_advance(ch, size) {
                pen += adv;
                continue;
            }
            let face = self.face_for(ch);
            let scaled = face.as_scaled(size);
            let gid = face.glyph_id(ch);
            let glyph = gid.with_scale_and_position(size, ab_glyph::point(pen, y));
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                if bounds.min.x as i32 >= clip.2 {
                    break;
                }
                let (ox, oy) = (bounds.min.x as i32, bounds.min.y as i32);
                outlined.draw(|gx, gy, cov| {
                    let py = oy + i32::try_from(gy).unwrap_or(i32::MAX);
                    // faux 이탤릭: 베이스라인 위로 갈수록 오른쪽으로 전단.
                    let shear = ((y - py as f32) * slant) as i32;
                    let base_px = ox + i32::try_from(gx).unwrap_or(i32::MAX) + shear;
                    for dx in 0..bold_pass {
                        let px = base_px + dx;
                        if px >= clip.0 && px < clip.2 && py >= clip.1 && py < clip.3 {
                            surface.blend_px(px, py, color, cov);
                        }
                    }
                });
            } else {
                // ★ 윤곽이 없는 글리프 = 비트맵 컬러 이모지 후보(T-18f). 볼드·이탤릭은 적용하지 않는다.
                self.draw_bitmap_glyph(surface, face, gid, pen, y, size, clip);
            }
            pen += scaled.h_advance(gid);
        }
        pen - x
    }

    /// `(x, y)`를 **베이스라인 왼쪽 끝**으로 텍스트를 그린다. 그린 폭(px)을 돌려준다.
    ///
    /// 커버리지를 배경과 블렌드(안티에일리어싱). 표면 밖은 [`Surface`]가 클립한다.
    pub fn draw_text(
        &self,
        surface: &mut Surface<'_>,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        text: &str,
    ) -> f32 {
        let mut pen = x;
        for ch in text.chars() {
            if let Some(adv) = self.control_advance(ch, size) {
                pen += adv;
                continue;
            }
            let face = self.face_for(ch);
            let scaled = face.as_scaled(size);
            let gid = face.glyph_id(ch);
            let glyph = gid.with_scale_and_position(size, ab_glyph::point(pen, y));
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let (ox, oy) = (bounds.min.x as i32, bounds.min.y as i32);
                outlined.draw(|gx, gy, cov| {
                    // 좌표 상한은 표면 클립이 보장 — i32 변환만 안전하게.
                    let px = ox + i32::try_from(gx).unwrap_or(i32::MAX);
                    let py = oy + i32::try_from(gy).unwrap_or(i32::MAX);
                    surface.blend_px(px, py, color, cov);
                });
            } else {
                // 윤곽 없음 → 비트맵 컬러 이모지 후보(T-18f). 클립 = 표면 전체.
                let whole = (
                    0,
                    0,
                    i32::try_from(surface.width()).unwrap_or(i32::MAX),
                    i32::try_from(surface.height()).unwrap_or(i32::MAX),
                );
                self.draw_bitmap_glyph(surface, face, gid, pen, y, size, whole);
            }
            pen += scaled.h_advance(gid);
        }
        pen - x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEJAVU: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
    const NOTO_EMOJI: &str = "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf";

    /// 윤곽 본(DejaVu) + 선택적 이모지 폴백. 파일이 없는 기기(다른 OS·최소 CI)는 `None` → 테스트 건너뜀.
    fn load(with_emoji: bool) -> Option<Font> {
        let mut font = Font::from_bytes(std::fs::read(DEJAVU).ok()?, 0).ok()?;
        if with_emoji {
            let data: &'static [u8] = Box::leak(std::fs::read(NOTO_EMOJI).ok()?.into_boxed_slice());
            font.push_fallback(data, 0).ok()?;
        }
        Some(font)
    }

    /// ★ T-18f — CBDT(PNG) 이모지가 **색으로** 그려지고, 폴백 판정·전진 폭이 일관된다.
    #[test]
    fn bitmap_emoji_draws_in_colour() {
        let (Some(plain), Some(font)) = (load(false), load(true)) else {
            eprintln!("건너뜀: DejaVu Sans 또는 Noto Color Emoji 없음");
            return;
        };
        let party = '\u{1F389}';
        assert!(!plain.covers(party), "윤곽 본만으로는 🎉가 없어야 한다");
        assert!(font.covers(party));
        assert!(font.has_glyph(party));
        // 어느 본에도 없는 코드포인트(비문자)는 여전히 "없음".
        assert!(!font.covers('\u{FFFFE}'));

        // ASCII는 이모지 폴백 유무와 무관하게 같은 폭.
        let size = 32.0;
        let ascii = "Hi ok";
        assert!((plain.measure(ascii, size) - font.measure(ascii, size)).abs() < f32::EPSILON);

        let (w, h) = (96usize, 64usize);
        let mut buf = vec![0u32; w * h];
        let mut surface = Surface::new(&mut buf, w, h);
        surface.fill(Color::from_rgb(255, 255, 255));
        let adv = font.draw_text_clipped(
            &mut surface,
            8.0,
            44.0,
            size,
            Color::from_rgb(0, 0, 0),
            "\u{1F389}",
            (0, 0, 96, 64),
        );
        assert!((adv - font.measure("\u{1F389}", size)).abs() < 0.01);
        assert!(adv > size * 0.5 && adv < size * 2.0, "전진 폭 {adv}");

        let (mut inked, mut coloured) = (0usize, 0usize);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        let mut distinct = std::collections::HashSet::new();
        for (i, &px) in buf.iter().enumerate() {
            let (r, g, b) = ((px >> 16) & 0xff, (px >> 8) & 0xff, px & 0xff);
            if (r, g, b) == (255, 255, 255) {
                continue;
            }
            inked += 1;
            distinct.insert(px & 0x00ff_ffff);
            let (x, y) = (i % w, i / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            // 채도가 뚜렷한 픽셀(회색 계열이 아님).
            if r.max(g).max(b) - r.min(g).min(b) > 60 {
                coloured += 1;
            }
        }
        eprintln!(
            "🎉 @{size}px: 전진 {adv:.2} · 칠해진 픽셀 {inked} · 유채색 {coloured} · 서로 다른 색 {} · 경계 x {x0}..={x1} y {y0}..={y1}",
            distinct.len()
        );
        assert!(inked > 200, "칠해진 픽셀 {inked}");
        assert!(coloured > 100, "유채색 픽셀 {coloured}");
        assert!(distinct.len() > 16, "색 종류 {}", distinct.len());
        // 펜(8) 근처에서 시작해 전진 폭 안에 들고, 베이스라인(44) 위아래에 걸친다.
        #[allow(clippy::cast_precision_loss)]
        {
            assert!(x0 >= 6 && (x1 as f32) <= 8.0 + adv + 2.0, "x {x0}..={x1}");
            assert!((y0 as f32) > 44.0 - size * 1.2 && (y1 as f32) < 44.0 + size * 0.5);
            assert!((y1 - y0) as f32 > size * 0.6, "높이 {}", y1 - y0);
        }

        // 클립 없는 경로(`draw_text`)도 같은 픽셀을 낸다.
        let mut buf2 = vec![0u32; w * h];
        let mut s2 = Surface::new(&mut buf2, w, h);
        s2.fill(Color::from_rgb(255, 255, 255));
        font.draw_text(
            &mut s2,
            8.0,
            44.0,
            size,
            Color::from_rgb(0, 0, 0),
            "\u{1F389}",
        );
        assert!(
            buf == buf2,
            "draw_text와 draw_text_clipped 결과가 달라선 안 된다"
        );
    }

    /// 클립 밖에는 비트맵 글리프도 찍지 않는다(볼드·이탤릭 플래그는 무시).
    #[test]
    fn bitmap_emoji_respects_clip() {
        let Some(font) = load(true) else {
            return;
        };
        let (w, h) = (96usize, 64usize);
        let mut buf = vec![0u32; w * h];
        let mut surface = Surface::new(&mut buf, w, h);
        surface.fill(Color::from_rgb(255, 255, 255));
        font.draw_styled(
            &mut surface,
            8.0,
            44.0,
            32.0,
            Color::from_rgb(0, 0, 0),
            "\u{1F389}",
            (0, 0, 24, 30),
            TextStyle {
                bold: true,
                italic: true,
            },
        );
        let mut inside = 0usize;
        for (i, &px) in buf.iter().enumerate() {
            if px & 0x00ff_ffff == 0x00ff_ffff {
                continue;
            }
            let (x, y) = (i % w, i / w);
            assert!(x < 24 && y < 30, "클립 밖 픽셀 ({x},{y})");
            inside += 1;
        }
        assert!(inside > 0, "클립 안에는 그려져야 한다");
    }
}
