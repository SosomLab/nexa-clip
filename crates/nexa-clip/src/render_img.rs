//! ★ 항목 → 비트맵 렌더(09-03 사용자 — "PPT에 붙여넣을 때 이미지처럼 나오면 좋겠어").
//!
//! 리치 런(색·굵기 · T-18d)을 자체 래스터라이저로 흰 바탕 비트맵에 그려, PNG(워커
//! 인코드) + `CF_DIBV5`로 게시할 재료를 만든다. 표시·기본 붙여넣기는 텍스트 그대로 —
//! 이 경로는 사용자가 "이미지로 복사"를 고를 때만 탄다.

use nclip_core::richtext::Run;
use nclip_gfx::{Color, Font, IconImage, Surface, TextStyle};

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

/// 런들을 흰 바탕 RGBA로 렌더 — ★ 탭 스톱 열맞춤(공백 4칸 격자) + ★ 2단 들여쓰기(em)·배율(줄 높이 = 줄의 최대 배율)
/// + ★ 인라인 이미지(`imgs` = (줄, 런) → 디코드본 · 원본 크기 · 폭 1200 상한).
pub(crate) fn render_runs(
    font: &Font,
    lines: &[Vec<Run>],
    imgs: &[((usize, usize), IconImage)],
) -> Option<(u32, u32, Vec<u8>)> {
    if lines.is_empty() {
        return None;
    }
    let base_h = (font.line_height(SIZE) * 1.15).ceil();
    let tab_w = font.measure("    ", SIZE).max(8.0);
    let em = font.measure("한", SIZE).max(8.0);
    let img_at = |li: usize, ri: usize| imgs.iter().find(|(k, _)| *k == (li, ri)).map(|(_, im)| im);
    #[allow(clippy::cast_precision_loss)]
    let fit = |im: &IconImage| -> (f32, f32) {
        let (iw, ih) = (im.w.max(1) as f32, im.h.max(1) as f32);
        let dw = iw.min(1200.0);
        (dw, (ih * dw / iw).max(1.0))
    };
    let line_h_of = |li: usize, line: &[Run]| -> f32 {
        let sc = line.iter().map(|r| r.scale).fold(1.0f32, f32::max);
        let mut h = (base_h * sc).ceil();
        for (ri, _) in line.iter().enumerate() {
            if let Some(im) = img_at(li, ri) {
                h = h.max(fit(im).1 + 6.0);
            }
        }
        h
    };
    let advance = |li: usize, line: &[Run]| -> f32 {
        let mut x = 0.0f32;
        for (ri, run) in line.iter().enumerate() {
            x += em * run.indent;
            if let Some(im) = img_at(li, ri) {
                x += fit(im).0;
                continue;
            }
            for (ti, seg) in run.text.split('\t').enumerate() {
                if ti > 0 {
                    x = ((x / tab_w).floor() + 1.0) * tab_w;
                }
                x += font.measure(seg, SIZE * run.scale);
            }
        }
        x
    };
    let max_w = lines
        .iter()
        .enumerate()
        .map(|(li, l)| advance(li, l))
        .fold(0.0f32, f32::max);
    let total_h: f32 = lines
        .iter()
        .enumerate()
        .map(|(li, l)| line_h_of(li, l))
        .sum();
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
    for (li, line) in lines.iter().enumerate() {
        let sc = line.iter().map(|r| r.scale).fold(1.0f32, f32::max);
        let line_h = line_h_of(li, line);
        let y = top + font.ascent(SIZE * sc);
        #[allow(clippy::cast_precision_loss)]
        let mut x = PAD as f32;
        for (ri, run) in line.iter().enumerate() {
            x += em * run.indent;
            if let Some(im) = img_at(li, ri) {
                let (dw, dh) = fit(im);
                #[allow(clippy::cast_possible_truncation)]
                surf.blend_image_scaled(
                    x.round() as i32,
                    (top + 3.0).round() as i32,
                    dw.round() as i32,
                    dh.round() as i32,
                    im,
                    clip,
                );
                x += dw;
                continue;
            }
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
                let sw = font.measure(seg, size);
                if let Some(b) = run.bg {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    surf.fill_rect(
                        x.round() as i32,
                        top.round() as i32,
                        sw.ceil().max(0.0) as u32,
                        line_h.ceil().max(0.0) as u32,
                        Color::from_rgb(b[0], b[1], b[2]),
                    );
                }
                font.draw_styled(&mut surf, x, y, size, col, seg, clip, style);
                x += sw;
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

/// 96dpi 기준 EMU/px — ONLYOFFICE 도형 자리(EMU)를 화소로 옮길 때 쓴다.
const EMU_PER_PX: f32 = 9525.0;

/// ★ 개체 그림들을 **원본 자리대로** 놓을 좌상단 좌표(px)를 구한다(10-04) — 못 믿겠으면 `None`.
///
/// `sizes` = 그림 크기(px · HTML 순서), `rects` = 도형 자리(EMU · [`nclip_core::richtext::onlyoffice_shape_rects`]).
/// 앞에서부터 그림 수만큼의 기록을 쓴다. 그림은 선 굵기·그림자만큼 도형보다 조금 크므로 **중심을 맞춘다**.
/// 그림/도형 크기 비(배율)가 서로 어긋나면(기록이 그 그림의 것이 아니다) 위치를 쓰지 않는다.
pub(crate) fn place_shapes(sizes: &[(u32, u32)], rects: &[[i32; 4]]) -> Option<Vec<(i32, i32)>> {
    if sizes.len() < 2 || rects.len() < sizes.len() {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let zooms: Vec<f32> = sizes
        .iter()
        .zip(rects)
        .map(|((w, _), r)| *w as f32 / (r[2] as f32 / EMU_PER_PX))
        .collect();
    let mut sorted = zooms.clone();
    sorted.sort_by(f32::total_cmp);
    let zoom = sorted[sorted.len() / 2];
    if !(0.25..=8.0).contains(&zoom) || zooms.iter().any(|z| !(zoom * 0.6..=zoom * 1.7).contains(z))
    {
        return None;
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let tl: Vec<(i32, i32)> = sizes
        .iter()
        .zip(rects)
        .map(|((w, h), r)| {
            let cx = (r[0] as f32 + r[2] as f32 / 2.0) / EMU_PER_PX * zoom;
            let cy = (r[1] as f32 + r[3] as f32 / 2.0) / EMU_PER_PX * zoom;
            (
                (cx - *w as f32 / 2.0).round() as i32,
                (cy - *h as f32 / 2.0).round() as i32,
            )
        })
        .collect();
    let (min_x, min_y) = tl.iter().fold((i32::MAX, i32::MAX), |(mx, my), (x, y)| {
        (mx.min(*x), my.min(*y))
    });
    Some(
        tl.into_iter()
            .map(|(x, y)| (x - min_x + PAD, y - min_y + PAD))
            .collect(),
    )
}

/// 그림들을 주어진 좌상단 자리에 **순서대로 겹쳐** 흰 바탕 RGBA 한 장으로(뒤의 것이 위).
pub(crate) fn compose_at(items: &[((i32, i32), &IconImage)]) -> Option<(u32, u32, Vec<u8>)> {
    #[allow(clippy::cast_possible_wrap)]
    let (w, h) = items.iter().fold((0i32, 0i32), |(w, h), ((x, y), im)| {
        (w.max(x + im.w as i32 + PAD), h.max(y + im.h as i32 + PAD))
    });
    if w <= 0 || h <= 0 || w > SIDE_MAX || h > SIDE_MAX || i64::from(w) * i64::from(h) > 16_000_000
    {
        return None;
    }
    #[allow(clippy::cast_sign_loss)]
    let (uw, uh) = (w as usize, h as usize);
    let mut out = vec![0xFFu8; uw * uh * 4];
    for ((x0, y0), im) in items {
        for sy in 0..im.h as usize {
            #[allow(clippy::cast_possible_wrap)]
            let dy = *y0 + sy as i32;
            if dy < 0 || dy >= h {
                continue;
            }
            for sx in 0..im.w as usize {
                #[allow(clippy::cast_possible_wrap)]
                let dx = *x0 + sx as i32;
                if dx < 0 || dx >= w {
                    continue;
                }
                let s = (sy * im.w as usize + sx) * 4;
                #[allow(clippy::cast_sign_loss)]
                let d = (dy as usize * uw + dx as usize) * 4;
                let a = u32::from(im.rgba[s + 3]);
                for c in 0..3 {
                    let v =
                        (u32::from(im.rgba[s + c]) * a + u32::from(out[d + c]) * (255 - a)) / 255;
                    #[allow(clippy::cast_possible_truncation)]
                    {
                        out[d + c] = v as u8;
                    }
                }
            }
        }
    }
    #[allow(clippy::cast_sign_loss)]
    Some((w as u32, h as u32, out))
}

/// RGBA → `CF_DIBV5`가 아닌 **`CF_DIB`(BITMAPINFOHEADER · 32bpp · 바텀업 BGRA)** —
/// PPT·Word가 가장 널리 받는 레거시 형태.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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
    /// ★ 개체 자리 — 도형 중심에 그림 중심을 맞추고, 배율이 어긋나면 쓰지 않는다(10-04).
    #[test]
    fn shapes_are_placed_by_their_centers() {
        // 실측(ONLYOFFICE 1p): 별 · 점선 사각형 · 글상자.
        let rects = [
            [1_469_385, 968_114, 2_389_057, 1_311_639],
            [2_929_191, 2_479_275, 2_857_500, 2_638_893],
            [4_014_590, 1_186_721, 4_175_124, 1_859_639],
        ];
        let sizes = [(251, 138), (306, 283), (438, 195)];
        let p = place_shapes(&sizes, &rects).expect("배율이 맞는다");
        // 별이 왼쪽 위, 사각형이 그 아래 오른쪽, 글상자가 사각형보다 위·오른쪽.
        assert!(p[0].0 < p[1].0 && p[0].1 < p[1].1, "{p:?}");
        assert!(p[2].1 < p[1].1 && p[2].0 > p[1].0, "{p:?}");
        assert!(p.iter().all(|(x, y)| *x >= PAD && *y >= PAD));
        // 그림 하나 · 기록 부족 · 배율 불일치는 None(가로 배치로 물러난다).
        assert!(place_shapes(&sizes[..1], &rects).is_none());
        assert!(place_shapes(&sizes, &rects[..2]).is_none());
        assert!(place_shapes(&[(251, 138), (3000, 283), (438, 195)], &rects).is_none());
    }

    #[test]
    fn compose_blends_on_white() {
        let red = IconImage::from_rgba(2, 2, [255u8, 0, 0, 255].repeat(4));
        let (w, h, px) = compose_at(&[((PAD, PAD), &red)]).expect("합성");
        assert_eq!((w, h), ((PAD * 2 + 2) as u32, (PAD * 2 + 2) as u32));
        assert_eq!(&px[..4], &[255, 255, 255, 255], "바탕은 흰색");
        let d = ((PAD as usize) * w as usize + PAD as usize) * 4;
        assert_eq!(&px[d..d + 4], &[255, 0, 0, 255]);
    }
}
