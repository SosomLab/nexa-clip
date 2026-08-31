//! 계열 아이콘 — ★ **코드로 그린다**(애셋 파일을 링크에 끌어들이지 않는다 · 단일 바이너리).
//!
//! 원래 트레이([`crate::tray_cmd`])만 쓰던 것을 창에도 붙인다(08-31 사용자 실기 —
//! *"작업표시줄에 일반 창 아이콘"*). Windows에서 작업표시줄·타이틀바 아이콘은
//! **창 아이콘**(`WM_SETICON`)에서 오는데 세 창(설정·팝업·데모) 모두 안 붙이고 있었다 —
//! Linux Dock 톱니바퀴(08-30 · `.desktop` 부재)와 같은 부류의 Windows판.
//!
//! | OS | 창 아이콘의 효과 |
//! |---|---|
//! | Windows | 타이틀바(小) + 작업표시줄(大 — `with_taskbar_icon`) |
//! | Linux/X11 | 태스크바·창 전환기 |
//! | Linux/Wayland | 무시 — `.desktop` + hicolor 아이콘이 담당(08-30 `install_launcher`) |
//! | macOS | 무시 — Dock 아이콘은 `.app` 번들 몫(T-9d) |

pub(crate) const ICON_SIDE: u32 = 32;

/// 라운드 스퀘어 + 청록 세로 그라디언트(`#22C3D6→#0B7FA6`) + 흰 클립보드 모티프.
pub(crate) fn icon_rgba() -> Vec<u8> {
    const TOP: (u8, u8, u8) = (0x22, 0xC3, 0xD6);
    const BOT: (u8, u8, u8) = (0x0B, 0x7F, 0xA6);
    let s = ICON_SIDE as i32;
    let radius = 7i32;
    let mut out = Vec::with_capacity((s * s * 4) as usize);
    for y in 0..s {
        // 세로 그라디언트.
        let t = y as u32;
        let lerp = |a: u8, b: u8| -> u8 {
            ((u32::from(a) * (ICON_SIDE - 1 - t) + u32::from(b) * t) / (ICON_SIDE - 1)) as u8
        };
        let (r, g, b) = (lerp(TOP.0, BOT.0), lerp(TOP.1, BOT.1), lerp(TOP.2, BOT.2));
        for x in 0..s {
            // 라운드 스퀘어 밖은 투명 — 네 모서리에서 반지름 검사.
            let cx = if x < radius {
                radius - 1 - x
            } else if x >= s - radius {
                x - (s - radius)
            } else {
                -1
            };
            let cy = if y < radius {
                radius - 1 - y
            } else if y >= s - radius {
                y - (s - radius)
            } else {
                -1
            };
            let outside = cx >= 0 && cy >= 0 && cx * cx + cy * cy > radius * radius;
            if outside {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // 흰 전경 — 클립보드 판(세로 직사각) + 상단 집게(가로 막대).
            let board = (8..24).contains(&x) && (10..26).contains(&y);
            let board_inner = (10..22).contains(&x) && (12..24).contains(&y);
            let clip = (12..20).contains(&x) && (6..11).contains(&y);
            if (board && !board_inner) || clip {
                out.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    out
}

/// 창 속성에 아이콘을 붙인다 — 모든 창 생성 지점이 이 한 곳을 지난다.
///
/// 변환 실패(크기 불일치 등)는 `None` = 아이콘 없이 진행(fail-soft — 안 뜨는 것보다 낫다).
pub(crate) fn with_icon(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    let icon = winit::window::Icon::from_rgba(icon_rgba(), ICON_SIDE, ICON_SIDE).ok();
    #[cfg(windows)]
    let attrs = {
        // Windows 작업표시줄은 큰 아이콘(`ICON_BIG`)을 따로 본다.
        use winit::platform::windows::WindowAttributesExtWindows as _;
        attrs.with_taskbar_icon(icon.clone())
    };
    attrs.with_window_icon(icon)
}
