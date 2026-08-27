//! 이미지 **머리글 해석 · 픽셀 변환 · 축소** — 전부 순수 함수(T-14c 1단).
//!
//! ## ★ 경계 원칙 — 압축 해제는 여기서 하지 않는다
//!
//! PNG·JPEG의 **압축 스트림 디코드**는 신뢰 못 할 입력의 공격면이라
//! **격리 워커**(`nclip-imgdec` — T-19 · beep `nbeep-imgdec` 선례: *"본체는 이미지
//! 파서를 절대 링크하지 않는다"*)로 간다. 여기 있는 것은 그보다 얕다:
//!
//! | 함수 | 읽는 것 | 왜 안전한가 |
//! |---|---|---|
//! | [`png_dimensions`] | 서명 8B + IHDR 폭·높이 | 고정 오프셋 · 압축 해제 없음 |
//! | [`dib_dimensions`] | `BITMAPINFOHEADER` 머리글 | 고정 오프셋 |
//! | [`dib_to_rgba`] | DIB **비압축** 픽셀(BI_RGB/BITFIELDS) | 압축 포맷은 거절(fail-soft) |
//! | [`downscale_rgba`] | 우리가 만든 RGBA | 입력이 이미 검증됨 |
//!
//! `dib_to_rgba`는 `nexa-beep` `crates/nbeep-plat/src/clipboard.rs`에서 이식했다
//! (순수 함수라 plat이 아니라 core에 둔다 — 3-OS 테스트 가능).

/// 변 상한 — 할당 폭탄 방어(beep `imgdec`와 같은 값).
const MAX_EDGE: u32 = 8192;
/// 픽셀 수 상한(≈16.7M).
const MAX_PIXELS: u64 = 16_777_216;

fn within_limits(w: u32, h: u32) -> bool {
    w > 0 && h > 0 && w <= MAX_EDGE && h <= MAX_EDGE && u64::from(w) * u64::from(h) <= MAX_PIXELS
}

/// PNG의 폭·높이 — **서명 + IHDR만** 읽는다(압축 해제 없음).
///
/// 형식: 서명 8B · 청크 길이 4B(=13) · `IHDR` · 폭 4B(BE) · 높이 4B(BE).
#[must_use]
pub fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if data.len() < 24 || data[..8] != SIG || &data[12..16] != b"IHDR" {
        return None;
    }
    let be = |o: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(o..o + 4)?.try_into().ok()?))
    };
    let (w, h) = (be(16)?, be(20)?);
    within_limits(w, h).then_some((w, h))
}

/// `CF_DIB`/`CF_DIBV5`의 폭·높이 — 머리글만 읽는다(픽셀은 안 만진다).
///
/// V5도 시작이 같은 배치라(`bi_size`만 124) 같은 오프셋으로 읽힌다.
#[must_use]
pub fn dib_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 40 {
        return None;
    }
    let le_i32 = |o: usize| -> Option<i32> {
        Some(i32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?))
    };
    let width = le_i32(4)?;
    let height_raw = le_i32(8)?;
    if width <= 0 || height_raw == 0 {
        return None;
    }
    let (w, h) = (width as u32, height_raw.unsigned_abs());
    within_limits(w, h).then_some((w, h))
}

/// `BMP` 파일(`BM` 머리글 14B + DIB)의 폭·높이.
#[must_use]
pub fn bmp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    (data.len() > 14 && data.starts_with(b"BM")).then(|| dib_dimensions(&data[14..]))?
}

/// 표현 이름에 맞는 치수 해석기를 골라 적용한다 — 모르는 포맷은 `None`.
///
/// ★ [`crate::capture::thumbnail_source`]가 고른 표현을 그대로 넣으면 된다.
#[must_use]
pub fn image_dimensions(format: &str, data: &[u8]) -> Option<(u32, u32)> {
    match format {
        "PNG" | "public.png" | "image/png" => png_dimensions(data),
        "CF_DIB" | "CF_DIBV5" => dib_dimensions(data),
        "image/bmp" => bmp_dimensions(data),
        _ => None,
    }
}

/// CF_DIB(BITMAPINFO) → RGBA(top-down) 변환 — **순수 함수**(전 OS 테스트).
///
/// 지원 = 24/32bpp × BI_RGB(0)·BI_BITFIELDS(3 · 표준 BGRA 마스크)만, 그 외
/// (팔레트·RLE·비표준 마스크)는 None(fail-soft). ⚠️ **알파는 255 고정** — 스크린샷
/// DIB의 알파 채널은 관행상 0이라 신뢰할 수 없다(beep 실측 기반 보수).
///
/// 이식 원본: `nexa-beep` `crates/nbeep-plat/src/clipboard.rs::dib_to_rgba`.
#[must_use]
pub fn dib_to_rgba(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    fn r_u32(b: &[u8], o: usize) -> Option<u32> {
        Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
    }
    fn r_i32(b: &[u8], o: usize) -> Option<i32> {
        Some(i32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
    }
    fn r_u16(b: &[u8], o: usize) -> Option<u16> {
        Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
    }
    let bi_size = r_u32(dib, 0)? as usize;
    if bi_size < 40 || dib.len() < bi_size {
        return None;
    }
    let width = r_i32(dib, 4)?;
    let height_raw = r_i32(dib, 8)?;
    let bit_count = r_u16(dib, 14)?;
    let compression = r_u32(dib, 16)?;
    if width <= 0 || height_raw == 0 {
        return None;
    }
    let w = width as u32;
    let top_down = height_raw < 0;
    let h = height_raw.unsigned_abs();
    if !within_limits(w, h) {
        return None;
    }
    let bytes_pp = match (bit_count, compression) {
        (32, 0) => 4,
        (32, 3) => {
            // 마스크는 헤더 40바이트 뒤(BITMAPINFOHEADER) 또는 V4/V5 헤더 안 —
            // 어느 쪽이든 시작에서 40..52. 표준 BGRA 배치만 받는다.
            let (r, g, b) = (r_u32(dib, 40)?, r_u32(dib, 44)?, r_u32(dib, 48)?);
            if (r, g, b) != (0x00FF_0000, 0x0000_FF00, 0x0000_00FF) {
                return None;
            }
            4
        }
        (24, 0) => 3,
        _ => return None,
    };
    let masks_extra = if bi_size == 40 && compression == 3 {
        12
    } else {
        0
    };
    let offset = bi_size + masks_extra;
    let stride = (w as usize * bytes_pp).div_ceil(4) * 4;
    let need = offset.checked_add(stride.checked_mul(h as usize)?)?;
    if dib.len() < need {
        return None;
    }
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for row_i in 0..h as usize {
        // DIB 기본은 bottom-up(양수 높이) — top-down으로 뒤집어 담는다.
        let src_row = if top_down {
            row_i
        } else {
            h as usize - 1 - row_i
        };
        let row = &dib[offset + src_row * stride..];
        for x in 0..w as usize {
            let px = &row[x * bytes_pp..];
            // DIB 픽셀은 BGR(A) 순서.
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    Some((w, h, rgba))
}

/// RGBA를 **긴 변이 `max_edge` 이하**가 되게 줄인다 — 박스 평균(영역 평균).
///
/// 이미 작으면 그대로 복제해 돌려준다(호출자가 분기하지 않아도 된다).
/// `data.len() != w*h*4`면 `None`(깨진 입력에 패닉 금지).
///
/// ★ 최근접 표집이 아니라 **박스 평균**인 이유 — 스크린샷을 1/10로 줄이면
/// 최근접은 가는 선·글자를 통째로 건너뛰어 썸네일이 "빈 화면"처럼 보인다.
#[must_use]
pub fn downscale_rgba(w: u32, h: u32, data: &[u8], max_edge: u32) -> Option<(u32, u32, Vec<u8>)> {
    if !within_limits(w, h) || max_edge == 0 {
        return None;
    }
    if data.len() != (w as usize) * (h as usize) * 4 {
        return None;
    }
    let long = w.max(h);
    if long <= max_edge {
        return Some((w, h, data.to_vec()));
    }
    // 목표 크기 — 비율 유지 · 최소 1px.
    let tw = ((u64::from(w) * u64::from(max_edge)) / u64::from(long)).max(1) as u32;
    let th = ((u64::from(h) * u64::from(max_edge)) / u64::from(long)).max(1) as u32;
    let mut out = Vec::with_capacity(tw as usize * th as usize * 4);
    for ty in 0..th {
        // 이 목표 픽셀이 덮는 원본 행 구간 [y0, y1).
        let y0 = (u64::from(ty) * u64::from(h) / u64::from(th)) as usize;
        let y1 = ((u64::from(ty) + 1) * u64::from(h) / u64::from(th)).max(y0 as u64 + 1) as usize;
        for tx in 0..tw {
            let x0 = (u64::from(tx) * u64::from(w) / u64::from(tw)) as usize;
            let x1 =
                ((u64::from(tx) + 1) * u64::from(w) / u64::from(tw)).max(x0 as u64 + 1) as usize;
            let (mut r, mut g, mut b, mut a) = (0u64, 0u64, 0u64, 0u64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w as usize + x) * 4;
                    r += u64::from(data[i]);
                    g += u64::from(data[i + 1]);
                    b += u64::from(data[i + 2]);
                    a += u64::from(data[i + 3]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            out.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]);
        }
    }
    Some((tw, th, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_header(w: u32, h: u32) -> Vec<u8> {
        let mut d = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        d.extend_from_slice(&13u32.to_be_bytes());
        d.extend_from_slice(b"IHDR");
        d.extend_from_slice(&w.to_be_bytes());
        d.extend_from_slice(&h.to_be_bytes());
        d.extend_from_slice(&[8, 6, 0, 0, 0]); // depth·color·… (치수 해석엔 무관)
        d
    }

    /// PNG 치수 — 서명·IHDR만 읽는다.
    #[test]
    fn png_dimensions_reads_ihdr_only() {
        assert_eq!(png_dimensions(&png_header(1920, 1080)), Some((1920, 1080)));
        assert!(png_dimensions(b"not a png").is_none());
        assert!(png_dimensions(&png_header(0, 10)).is_none(), "0폭 거절");
        assert!(
            png_dimensions(&png_header(9000, 10)).is_none(),
            "변 상한 8192"
        );
    }

    /// DIB 치수 — 음수 높이(top-down)는 절댓값.
    #[test]
    fn dib_dimensions_handles_top_down() {
        let mut d = vec![0u8; 40];
        d[0] = 40;
        d[4..8].copy_from_slice(&320i32.to_le_bytes());
        d[8..12].copy_from_slice(&(-240i32).to_le_bytes());
        assert_eq!(dib_dimensions(&d), Some((320, 240)));
        assert!(dib_dimensions(&[0u8; 10]).is_none());
    }

    /// 이름 → 해석기 연결([`crate::capture::thumbnail_source`]와 맞물린다).
    #[test]
    fn dimensions_dispatch_by_format_name() {
        assert_eq!(
            image_dimensions("PNG", &png_header(4, 2)),
            Some((4, 2)),
            "PNG"
        );
        assert!(image_dimensions("Art::GVML ClipFormat", &[0; 64]).is_none());
    }

    // ── dib_to_rgba — beep 이식 테스트 그대로 ─────────────────────────

    /// 32bpp BI_RGB 2x2 bottom-up — BGR(A)→RGBA 변환 + 행 뒤집기.
    #[test]
    fn dib_32bpp_bottom_up() {
        let mut d = vec![0u8; 40];
        d[0] = 40;
        d[4..8].copy_from_slice(&2i32.to_le_bytes());
        d[8..12].copy_from_slice(&2i32.to_le_bytes());
        d[14..16].copy_from_slice(&32u16.to_le_bytes());
        // 픽셀(BGRA · bottom-up): 아래 행 = [파랑, 초록] · 위 행 = [빨강, 흰].
        d.extend_from_slice(&[255, 0, 0, 0, 0, 255, 0, 0]);
        d.extend_from_slice(&[0, 0, 255, 0, 255, 255, 255, 0]);
        let (w, h, rgba) = dib_to_rgba(&d).expect("파싱");
        assert_eq!((w, h), (2, 2));
        // top-down 결과 첫 행 = 위 행(빨강·흰) — 알파는 255 고정.
        assert_eq!(&rgba[0..8], &[255, 0, 0, 255, 255, 255, 255, 255]);
        assert_eq!(&rgba[8..16], &[0, 0, 255, 255, 0, 255, 0, 255]);
    }

    /// 24bpp — stride 4바이트 정렬 패딩 + top-down(음수 높이).
    #[test]
    fn dib_24bpp_stride_and_top_down() {
        let mut d = vec![0u8; 40];
        d[0] = 40;
        d[4..8].copy_from_slice(&1i32.to_le_bytes());
        d[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        d[14..16].copy_from_slice(&24u16.to_le_bytes());
        d.extend_from_slice(&[10, 20, 30, 0]); // BGR + 패딩 1
        d.extend_from_slice(&[40, 50, 60, 0]);
        let (w, h, rgba) = dib_to_rgba(&d).expect("파싱");
        assert_eq!((w, h), (1, 2));
        assert_eq!(&rgba[..], &[30, 20, 10, 255, 60, 50, 40, 255]);
    }

    /// 미지원(팔레트)·손상·상한 초과 = None(fail-soft).
    #[test]
    fn dib_rejects_unsupported() {
        assert!(dib_to_rgba(&[0u8; 10]).is_none(), "헤더 미달");
        let mut pal = vec![0u8; 40];
        pal[0] = 40;
        pal[4..8].copy_from_slice(&2i32.to_le_bytes());
        pal[8..12].copy_from_slice(&2i32.to_le_bytes());
        pal[14..16].copy_from_slice(&8u16.to_le_bytes());
        assert!(dib_to_rgba(&pal).is_none(), "팔레트 미지원");
        let mut big = vec![0u8; 40];
        big[0] = 40;
        big[4..8].copy_from_slice(&9000i32.to_le_bytes());
        big[8..12].copy_from_slice(&2i32.to_le_bytes());
        big[14..16].copy_from_slice(&32u16.to_le_bytes());
        assert!(dib_to_rgba(&big).is_none(), "변 상한 8192");
    }

    // ── downscale ─────────────────────────────────────────────────────

    /// 2×2 → 1×1 — 네 픽셀의 **평균**이 나온다(박스 평균의 정의 그 자체).
    #[test]
    fn downscale_averages_not_samples() {
        let data = [
            0, 0, 0, 255, 255, 255, 255, 255, //
            255, 255, 255, 255, 0, 0, 0, 255,
        ];
        let (w, h, out) = downscale_rgba(2, 2, &data, 1).expect("축소");
        assert_eq!((w, h), (1, 1));
        assert_eq!(
            &out[..],
            &[127, 127, 127, 255],
            "최근접이면 0 또는 255가 나온다"
        );
    }

    /// 이미 작으면 그대로 — 비율은 항상 유지된다.
    #[test]
    fn downscale_keeps_small_and_aspect() {
        let px = vec![9u8; 4 * 4 * 2 * 4];
        let (w, h, out) = downscale_rgba(4, 8, &px, 16).expect("그대로");
        assert_eq!((w, h), (4, 8));
        assert_eq!(out.len(), px.len());
        // 400×100 → 긴 변 40 = 40×10.
        let wide = vec![1u8; 400 * 100 * 4];
        let (w, h, _) = downscale_rgba(400, 100, &wide, 40).expect("축소");
        assert_eq!((w, h), (40, 10));
    }

    /// 깨진 입력엔 None — 패닉 금지(클립보드는 남의 데이터다).
    #[test]
    fn downscale_rejects_bad_input() {
        assert!(downscale_rgba(2, 2, &[0u8; 3], 1).is_none(), "길이 불일치");
        assert!(downscale_rgba(0, 2, &[], 1).is_none());
        assert!(downscale_rgba(2, 2, &[0u8; 16], 0).is_none(), "0 상한");
    }
}
