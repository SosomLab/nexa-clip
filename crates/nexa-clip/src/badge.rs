//! ★ 트레이 메뉴 **순번 배지**(10-17 사용자 — *"열기 메뉴 위의 클립보드 내용에 순번도 같이 · 이미지 공간을 활용"*).
//!
//! 호스트가 번호를 **RGBA 그림으로 그려** [`nclip_plat::tray::TrayContent::recent_badges`]에 싣고, OS 어댑터가
//! 메뉴 아이콘 칸에 붙인다(Windows `hbmpItem` · mac `NSMenuItem.image` · Linux dbusmenu `icon-data`). 라벨 글에
//! "1. "을 붙이는 대신 아이콘 칸을 쓰므로 세 OS 메뉴가 같은 모양이 된다(DR-1의 트레이판).
//!
//! 그림 = 둥근 원(앱 아이콘 청록 계열) + 흰 숫자(UI 글꼴). 래스터는 `nexa_gfx::Surface`(0RGB · 알파 없음)라
//! 숫자를 **검은 바탕에 흰색**으로 한 번 그려 덮임(coverage)을 얻고, 원의 덮임은 해석적으로 구해 합성한다.

use nexa_gfx::{Color, Font, Surface, TextStyle};

/// 배지 바탕색 — 앱 아이콘 그라디언트 아래쪽(`#0B7FA6`)·흰 숫자와 대비.
const BG: (u8, u8, u8) = (0x0B, 0x7F, 0xA6);

/// 배지 한 변(px) — Windows 메뉴 비트맵은 1:1로 그려져 **DPI 실측 크기**(16px × 배율)여야 하고,
/// mac·Linux는 셸이 자기 크기로 줄이므로 넉넉히 32px.
#[must_use]
pub(crate) fn side_for(scale: f64) -> u32 {
    if cfg!(windows) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let s = (16.0 * scale.max(0.5)).round() as u32;
        s.clamp(12, 64)
    } else {
        32
    }
}

/// 중심 `(cx, cy)` · 반지름 `r`인 원이 화소 `(x, y)`를 덮는 비율(0..=1) — 4×4 표본 상자 필터.
#[must_use]
pub(crate) fn disc_coverage(cx: f32, cy: f32, r: f32, x: u32, y: u32) -> f32 {
    let mut hit = 0u32;
    for sy in 0..4 {
        for sx in 0..4 {
            #[allow(clippy::cast_precision_loss)]
            let (px, py) = (
                x as f32 + (sx as f32 + 0.5) / 4.0,
                y as f32 + (sy as f32 + 0.5) / 4.0,
            );
            let (dx, dy) = (px - cx, py - cy);
            if dx * dx + dy * dy <= r * r {
                hit += 1;
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    {
        hit as f32 / 16.0
    }
}

/// 번호 `n`(1부터)의 배지 RGBA(straight · `side`×`side`). 글꼴이 숫자를 못 그려도 원은 그려진다.
#[must_use]
pub(crate) fn render(font: &Font, n: usize, side: u32) -> Vec<u8> {
    let side = side.clamp(8, 256);
    let s = side as usize;
    let text = n.to_string();
    // 숫자 덮임 — 검은 바탕에 흰 글자(0RGB의 R 채널 = 덮임).
    let mut buf = vec![0u32; s * s];
    {
        let mut surf = Surface::new(&mut buf, s, s);
        #[allow(clippy::cast_precision_loss)]
        let sidef = side as f32;
        // 굵게 — 16px 배지에서 가는 숫자는 흐릿하다(덤프 확인 10-17).
        let style = TextStyle {
            bold: true,
            italic: false,
        };
        let size = if text.len() >= 2 {
            sidef * 0.58
        } else {
            sidef * 0.72
        };
        let tw = font.measure_from_styled(&text, size, 0.0, true);
        let lh = font.line_height(size);
        let x = ((sidef - tw) / 2.0).max(0.0);
        let y = (sidef - lh) / 2.0 + font.ascent(size);
        #[allow(clippy::cast_possible_wrap)]
        let clip = (0, 0, side as i32, side as i32);
        font.draw_styled(
            &mut surf,
            x,
            y,
            size,
            Color::from_rgb(255, 255, 255),
            &text,
            clip,
            style,
            x,
        );
    }
    #[allow(clippy::cast_precision_loss)]
    let (c, r) = (side as f32 / 2.0, side as f32 / 2.0 - 0.5);
    let mut out = Vec::with_capacity(s * s * 4);
    for y in 0..side {
        for x in 0..side {
            let cov = disc_coverage(c, c, r, x, y);
            #[allow(clippy::cast_precision_loss)]
            let t = f32::from((buf[(y as usize) * s + x as usize] >> 16) as u8) / 255.0;
            // 원 안에서 글자 덮임만큼 흰색으로.
            let mix = |b: u8| -> u8 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    (f32::from(b) * (1.0 - t) + 255.0 * t)
                        .round()
                        .clamp(0.0, 255.0) as u8
                }
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (cov * 255.0).round() as u8;
            out.extend_from_slice(&[mix(BG.0), mix(BG.1), mix(BG.2), a]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows = 16px × 배율(12~64 클램프) · 다른 OS = 32.
    #[test]
    fn side_follows_scale_on_windows_only() {
        if cfg!(windows) {
            assert_eq!(side_for(1.0), 16);
            assert_eq!(side_for(1.5), 24);
            assert_eq!(side_for(2.0), 32);
            assert_eq!(side_for(10.0), 64);
        } else {
            assert_eq!(side_for(1.0), 32);
            assert_eq!(side_for(2.0), 32);
        }
    }

    /// ★ 눈으로 보는 덤프(기본 제외) — `cargo test -p nexa-clip badge::tests::dump -- --ignored` →
    ///   `target/badge-dump/badge-<변>-<n>.png`(시스템 UI 글꼴 · 16/24/32px · 1~10).
    #[test]
    #[ignore = "시각 확인용 — 파일을 쓴다"]
    fn dump_badges_to_png() {
        let Some(sys) = nexa_font::system_ui_font() else {
            return;
        };
        let Ok(font) = Font::from_static(sys.data, sys.index) else {
            return;
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/badge-dump");
        std::fs::create_dir_all(&dir).expect("폴더");
        for side in [16u32, 24, 32] {
            for n in 1..=10usize {
                let rgba = render(&font, n, side);
                let png = nclip_core::img::png_encode_rgba(side, side, &rgba).expect("png");
                std::fs::write(dir.join(format!("badge-{side}-{n}.png")), png).expect("쓰기");
            }
        }
    }

    /// 원 덮임 — 중심은 1 · 모서리는 0 · 경계는 그 사이.
    #[test]
    fn disc_coverage_is_antialiased() {
        assert!((disc_coverage(8.0, 8.0, 7.5, 8, 8) - 1.0).abs() < f32::EPSILON);
        assert!(disc_coverage(8.0, 8.0, 7.5, 0, 0).abs() < f32::EPSILON);
        let edge = disc_coverage(8.0, 8.0, 7.5, 15, 8);
        assert!(edge > 0.0 && edge < 1.0, "{edge}");
    }
}
