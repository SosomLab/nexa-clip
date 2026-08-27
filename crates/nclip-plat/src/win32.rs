//! 공용 Win32 선언 — ★ **crate 안에서 겹치는 것은 전부 여기 한 곳에만 둔다**.
//!
//! ⚠️ 같은 함수를 다른 시그니처(다른 타입 별칭·다른 구조체 pointee 포함)로 두 번
//! 선언하면 `clashing_extern_declarations`(우리 설정에서는 **오류**)다. `watch_win`의
//! *"같은 선언이어야 한다"* 경고가 트레이 이식(T-12e)에서 실제로 터져 이 모듈로 모았다.
//!
//! 핸들 규약 = **`isize`**(paste.rs가 세운 crate 규약 — beep은 `*mut c_void`지만
//! 우리는 기존 코드를 따른다). 한 모듈만 쓰는 함수는 그 모듈에 둔다(여기는 겹침 방지용).

// ★ Win32 타입 이름은 원문 그대로(MSDN 대조) — FFI 선언부에 한해 린트를 끈다.
#![allow(clippy::upper_case_acronyms)]

/// 창 핸들.
pub(crate) type HWND = isize;
/// 일반 핸들(모듈·GDI 객체 등).
pub(crate) type HANDLE = isize;
pub(crate) type WPARAM = usize;
pub(crate) type LPARAM = isize;
pub(crate) type LRESULT = isize;

/// 창 프로시저 포인터.
pub(crate) type WndProc = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

#[repr(C)]
pub(crate) struct WNDCLASSW {
    pub(crate) style: u32,
    pub(crate) lpfn_wnd_proc: WndProc,
    pub(crate) cb_cls_extra: i32,
    pub(crate) cb_wnd_extra: i32,
    pub(crate) h_instance: HANDLE,
    pub(crate) h_icon: HANDLE,
    pub(crate) h_cursor: HANDLE,
    pub(crate) hbr_background: HANDLE,
    pub(crate) lpsz_menu_name: *const u16,
    pub(crate) lpsz_class_name: *const u16,
}

#[repr(C)]
pub(crate) struct MSG {
    pub(crate) hwnd: HWND,
    pub(crate) message: u32,
    pub(crate) w_param: WPARAM,
    pub(crate) l_param: LPARAM,
    pub(crate) time: u32,
    pub(crate) pt_x: i32,
    pub(crate) pt_y: i32,
}

#[link(name = "user32")]
extern "system" {
    pub(crate) fn RegisterClassW(cls: *const WNDCLASSW) -> u16;
    pub(crate) fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: isize,
        menu: isize,
        inst: isize,
        param: *const core::ffi::c_void,
    ) -> isize;
    pub(crate) fn DefWindowProcW(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    pub(crate) fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> i32;
    pub(crate) fn TranslateMessage(msg: *const MSG) -> i32;
    pub(crate) fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    pub(crate) fn PostMessageW(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> i32;
    pub(crate) fn PostQuitMessage(code: i32);
    pub(crate) fn SetForegroundWindow(hwnd: HWND) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    pub(crate) fn GetModuleHandleW(name: *const u16) -> HANDLE;
}
