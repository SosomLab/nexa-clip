//! ★ 항목 → 비트맵 렌더(09-03 사용자 — "PPT에 붙여넣을 때 이미지처럼 나오면 좋겠어").
//!
//! 리치 런(색·굵기 · T-18d)을 자체 래스터라이저로 흰 바탕 비트맵에 그려, PNG(워커
//! 인코드) + `CF_DIBV5`로 게시할 재료를 만든다. 표시·기본 붙여넣기는 텍스트 그대로 —
//! 이 경로는 사용자가 "이미지로 복사"를 고를 때만 탄다.

use nclip_core::richtext::Run;
use nclip_gfx::{Color, Font, Surface, TextStyle};

/// 렌더 글자 크기(px) — PPT 슬라이드 대비 적당한 밀도.
const SIZE: f32 = 18.0;
/// 사방 여백(px).
const PAD: i32 = 16;
/// 캔버스 상한 — 총화소 16M(RGBA 64MiB) · 변 4000px.
const SIDE_MAX: i32 = 4000;

/// 평문 → 스타일 없는 런(줄 500개 상한).
pub(crate) fn plain_runs(text: &str) -> Vec<Vec<Run>> {
    text.lines()
        .take(500)
        .map(|l| {
            vec![Run {
                text: l.to_string(),
                ..Run::default()
            }]
        })
        .collect()
}

/// 런들을 흰 바탕 RGBA로 렌더 — ★ 탭 스톱 열맞춤(공백 4칸 격자) + ★ 2단 들여쓰기(em)·배율(줄 높이 = 줄의 최대 배율).
pub(crate) fn render_runs(font: &Font, lines: &[Vec<Run>]) -> Option<(u32, u32, Vec<u8>)> {
    if lines.is_empty() {
        return None;
    }
    let base_h = (font.line_height(SIZE) * 1.15).ceil();
    let tab_w = font.measure("    ", SIZE).max(8.0);
    let em = font.measure("한", SIZE).max(8.0);
    let line_scale = |line: &[Run]| line.iter().map(|r| r.scale).fold(1.0f32, f32::max);
    let advance = |x: f32, line: &[Run]| -> f32 {
        let mut x = x;
        for run in line {
            x += em * run.indent;
            for (ti, seg) in run.text.split('\t').enumerate() {
                if ti > 0 {
                    x = ((x / tab_w).floor() + 1.0) * tab_w;
                }
                x += font.measure(seg, SIZE * run.scale);
            }
        }
        x
    };
    let max_w = lines.iter().map(|l| advance(0.0, l)).fold(0.0f32, f32::max);
    let total_h: f32 = lines.iter().map(|l| (base_h * line_scale(l)).ceil()).sum();
    #[allow(clippy::cast_possible_truncation)]
    let w = (max_w.ceil() as i32 + PAD * 2).clamp(40, SIDE_MAX);
    #[allow(clippy::cast_possible_truncation)]
    let h = (total_h.ceil() as i32 + PAD * 2).clamp(30, SIDE_MAX);
    if i64::from(w) * i64::from(h) > 16_000_000 {
        return None;
    }
    let (uw, uh) = (w as usize, h as usize);
    let mut buf = vec![0u32; uw * uh];
    let mut surf = Surface::new(&mut buf, uw, uh);
    surf.fill_rect(0, 0, w as u32, h as u32, Color::from_rgb(255, 255, 255));
    let clip = (0, 0, w, h);
    #[allow(clippy::cast_precision_loss)]
    let mut top = PAD as f32;
    for line in lines {
        let sc = line_scale(line);
        let line_h = (base_h * sc).ceil();
        let y = top + font.ascent(SIZE * sc);
        #[allow(clippy::cast_precision_loss)]
        let mut x = PAD as f32;
        for run in line {
            x += em * run.indent;
            let size = SIZE * run.scale;
            let col = run.color.map_or(Color::from_rgb(20, 20, 20), |c| {
                Color::from_rgb(c[0], c[1], c[2])
            });
            let style = TextStyle {
                bold: run.bold,
                italic: run.italic,
            };
            for (ti, seg) in run.text.split('\t').enumerate() {
                if ti > 0 {
                    #[allow(clippy::cast_precision_loss)]
                    let rel = x - PAD as f32;
                    #[allow(clippy::cast_precision_loss)]
                    {
                        x = PAD as f32 + ((rel / tab_w).floor() + 1.0) * tab_w;
                    }
                }
                if seg.is_empty() {
                    continue;
                }
                font.draw_styled(&mut surf, x, y, size, col, seg, clip, style);
                x += font.measure(seg, size);
            }
        }
        top += line_h;
    }
    // 0RGB u32 → RGBA(불투명).
    let mut rgba = Vec::with_capacity(uw * uh * 4);
    for px in &buf {
        rgba.extend_from_slice(&[(px >> 16) as u8, (px >> 8) as u8, *px as u8, 0xFF]);
    }
    #[allow(clippy::cast_sign_loss)]
    Some((w as u32, h as u32, rgba))
}

/// RGBA → `CF_DIBV5`가 아닌 **`CF_DIB`(BITMAPINFOHEADER · 32bpp · 바텀업 BGRA)** —
/// PPT·Word가 가장 널리 받는 레거시 형태.
pub(crate) fn dib_from_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(40 + rgba.len());
    let px = u64::from(w) * u64::from(h);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // 양수 = 바텀업
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((px * 4) as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // ppm×2 · clrUsed · clrImportant
    for row in (0..h).rev() {
        let base = (row as usize) * (w as usize) * 4;
        for col in 0..w as usize {
            let i = base + col * 4;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], 0xFF]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DIB 헤더·크기 계약 — 40B 헤더 + w×h×4.
    #[test]
    fn dib_layout() {
        let rgba = vec![0u8; 2 * 2 * 4];
        let dib = dib_from_rgba(2, 2, &rgba);
        assert_eq!(dib.len(), 40 + 16);
        assert_eq!(&dib[..4], &40u32.to_le_bytes());
        assert_eq!(&dib[14..16], &32u16.to_le_bytes());
    }
}
