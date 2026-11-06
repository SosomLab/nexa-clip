//! ★ 창 투명도(11-06 · nexa-dir3 T-102 · NEW-013 · T-199 이식) — **창 전체를 OS 합성으로 흐리게** 하는 플랫폼 경계.
//!
//! - Windows = 레이어드 창 알파(`WS_EX_LAYERED` + `SetLayeredWindowAttributes(LWA_ALPHA)`) · 불투명(255)이면 레이어드를
//!   떼어 종전 그대로(합성 비용 0).
//! - macOS = `NSWindow alphaValue`(NSView → window · `msg_send!` 직접 — objc2 feature 추가 없음).
//! - Linux X11(XWayland 포함) = `_NET_WM_WINDOW_OPACITY`(CARDINAL · 합성 WM이 반영) — **지속 연결 + `check()` 동기 확인**.
//!   dir3 11-01 GNOME 실기로 확정한 결함: 매번 새 연결을 열어 쓰고 바로 닫으면 연결 종료가 버퍼의 요청 처리와 경합해
//!   쓰기가 가끔 **버려졌다**(Alt+Tab처럼 빠른 전환에서 잦음). 같은 연결에서 응답을 기다린 뒤에는 36/36 반영.
//!   Wayland 네이티브 = 프로토콜에 창 알파 요청이 없다 → `Err`(호출부는 조용히 넘긴다 · 최상위 고정과 같은 결).
//!
//! 핸들은 본체(`nexa-clip/src/transparency.rs`)가 winit `raw_window_handle`로 꺼내 넘긴다 — 이 crate는 winit을 모른다
//! (GAP-042 `wintaskbar`와 같은 분담). 순수 변환([`opacity_alpha`])은 3-OS 공통이라 시험이 어디서든 돈다.
//!
//! 출처 = `../nexa-dir3/crates/nexa-dir/src/platform/mod.rs`(창 투명도 절 · `windows` 크레이트 판) → 이 crate 규약
//! (DR-8 외부 crate 0 · 순수 `extern "system"` · 핸들 `isize`)으로 옮겼다.

/// 투명도 상한(%) — 그 이상은 창이 거의 안 보여 다시 끌 수 없게 된다(dir3 DR-22 0~90).
pub const OPACITY_MAX_PCT: i64 = 90;

/// 투명도(%) → 불투명도 0~255(순수 · 0 % = 255 · 90 % = 25 · 범위 밖은 클램프).
#[must_use]
pub fn opacity_alpha(pct: i64) -> u8 {
    let p = pct.clamp(0, OPACITY_MAX_PCT);
    (255 - (255 * p + 50) / 100) as u8
}

/// Windows — 레이어드 창 알파. `hwnd` = 살아 있는 최상위 창 · UI 스레드에서 부른다. 반환 = 성공 여부.
#[cfg(windows)]
#[must_use]
pub fn set_hwnd_alpha(hwnd: isize, alpha: u8) -> bool {
    use crate::win32::{GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW};
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const LWA_ALPHA: u32 = 0x0000_0002;
    if hwnd == 0 {
        return false;
    }
    // SAFETY: 살아 있는 winit 창의 HWND · 확장 스타일 읽고 쓰기 · 레이어드 속성은 알파만.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if alpha == 255 {
            // 불투명 = 레이어드를 떼어 종전 그대로(합성 비용 0).
            if ex & WS_EX_LAYERED != 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex & !WS_EX_LAYERED);
            }
            return true;
        }
        if ex & WS_EX_LAYERED == 0 {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
        }
        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) != 0
    }
}

/// macOS — NSView가 속한 `NSWindow`의 `alphaValue`(0.0~1.0). 창이 없으면(아직 안 붙음) `false`. 메인 스레드에서 부른다.
#[cfg(target_os = "macos")]
#[must_use]
pub fn set_ns_view_window_alpha(ns_view: *mut core::ffi::c_void, alpha: f64) -> bool {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    if ns_view.is_null() {
        return false;
    }
    let view = ns_view.cast::<AnyObject>();
    // SAFETY: 살아 있는 winit 창의 NSView · `window`는 NSWindow 또는 nil · `setAlphaValue:`는 CGFloat(f64) 하나.
    unsafe {
        let win: *mut AnyObject = msg_send![view, window];
        if win.is_null() {
            return false;
        }
        let _: () = msg_send![win, setAlphaValue: alpha.clamp(0.0, 1.0)];
    }
    true
}

/// Linux X11 — `_NET_WM_WINDOW_OPACITY`(CARDINAL · `alpha × 0x01010101` · 255면 속성 삭제). 연결은 프로세스에 하나 ·
/// 처음 쓸 때 열고, 요청이 실패하면(X 재시작 · 끊김) 버리고 다음에 다시 연다. `DISPLAY`가 없으면(Wayland 네이티브) `Err`.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn set_x11_opacity(win: u32, alpha: u8) -> Result<(), String> {
    x11_link::set_opacity(win, alpha)
}

#[cfg(all(unix, not(target_os = "macos")))]
mod x11_link {
    use std::sync::Mutex;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    struct Link {
        conn: RustConnection,
        opacity: u32,
    }

    static LINK: Mutex<Option<Link>> = Mutex::new(None);

    fn open() -> Result<Link, String> {
        if !std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
            return Err("DISPLAY 없음(Wayland 네이티브)".into());
        }
        let (conn, _) = RustConnection::connect(None).map_err(|e| e.to_string())?;
        let opacity = conn
            .intern_atom(false, b"_NET_WM_WINDOW_OPACITY")
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .atom;
        Ok(Link { conn, opacity })
    }

    fn write(l: &Link, win: u32, alpha: u8) -> Result<(), String> {
        if alpha == 255 {
            l.conn
                .delete_property(win, l.opacity)
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())
        } else {
            let v = u32::from(alpha) * 0x0101_0101;
            l.conn
                .change_property32(PropMode::REPLACE, win, l.opacity, AtomEnum::CARDINAL, &[v])
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())
        }
    }

    pub(super) fn set_opacity(win: u32, alpha: u8) -> Result<(), String> {
        let mut guard = LINK.lock().map_err(|_| "X11 link poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(open()?);
        }
        let Some(l) = guard.as_ref() else {
            return Err("no X11 connection".into());
        };
        match write(l, win, alpha) {
            Ok(()) => Ok(()),
            Err(e) => {
                // 끊긴 연결은 버린다 — 다음 호출이 다시 연다(X 재시작 · 소켓 오류).
                *guard = None;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 투명도 % → 알파 경계(dir3 `window_transparency_follows_setting_and_boost` 순수부).
    #[test]
    fn opacity_alpha_bounds_and_rounding() {
        assert_eq!(opacity_alpha(0), 255);
        assert_eq!(opacity_alpha(-5), 255);
        assert_eq!(opacity_alpha(50), 127);
        assert_eq!(opacity_alpha(90), 25);
        // 상한 넘김은 90으로 클램프 — 창이 사라져 되돌릴 수 없는 값은 만들지 않는다.
        assert_eq!(opacity_alpha(100), 25);
        assert_eq!(opacity_alpha(5), 242);
        assert_eq!(opacity_alpha(10), 229);
    }
}
