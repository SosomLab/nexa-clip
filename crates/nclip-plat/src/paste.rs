//! 포커스 복원 + 키 주입 어댑터 — ★ **K-1 스파이크**([docs/02 §7](../../../docs/02-roadmap.md)).
//!
//! 외부 crate를 쓰지 않는다(DR-8) — OS 함수를 `#[link]` + `extern`으로 직접 선언한다.
//!
//! | OS | 상태 | 구현 |
//! |---|---|---|
//! | **Windows** | ✅ 구현 | `GetForegroundWindow` → `AttachThreadInput` + `SetForegroundWindow` → `SendInput` |
//! | **macOS** | 🚧 구현(실기 미검증) | `CGEventPost`(⌘V) + `AXIsProcessTrusted` 권한 확인 |
//! | **Linux** | ✅ 구현(08-30) | **X11 `XTest`**(x11rb 순수 Rust) — X11 세션은 `GetInputFocus`→`_NET_ACTIVE_WINDOW`+주입 · **Wayland 세션은 XWayland의 XTest**(Mutter가 가상 입력 장치로 넘긴다) + 포커스 복원은 **컴포지터가 팝업을 닫을 때 돌려준다**(클라이언트는 남의 창을 활성화할 수 없다 — 그래서 **팝업을 먼저 닫는 순서**가 전부다) · `DISPLAY` 없으면 정직히 `WaylandNoInjection` |
//!
//! ## ⚠️ Windows에서 포커스를 되돌리는 건 그냥 안 된다
//!
//! `SetForegroundWindow`는 **아무 프로세스나 부를 수 없다**(포그라운드 강탈 방지).
//! 우리 스레드를 대상 창의 입력 큐에 **붙였다가**(`AttachThreadInput`) 떼는 고전 우회가 필요하다.
//! 이 트릭이 없으면 창이 **깜빡이기만 하고 포커스가 안 간다**.

use nclip_core::{PasteAs, PasteCapability, PasteError, PasteInjector, PasteUnsupported};

/// 이 타깃의 붙여넣기 어댑터.
#[derive(Debug, Default)]
pub struct PlatformPaste {
    target: Option<imp::Target>,
}

impl PlatformPaste {
    /// 새 어댑터.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 기억해 둔 대상의 사람이 읽을 수 있는 이름(진단·스파이크 표시용).
    #[must_use]
    pub fn target_label(&self) -> Option<String> {
        self.target.as_ref().map(imp::label)
    }
}

impl PasteInjector for PlatformPaste {
    fn capability(&self) -> PasteCapability {
        imp::capability()
    }

    fn capture_focus(&mut self) -> bool {
        self.target = imp::foreground();
        self.target.is_some()
    }

    fn restore_and_paste(&mut self, as_: PasteAs) -> Result<(), PasteError> {
        let Some(target) = self.target.as_ref() else {
            return Err(PasteError::TargetGone);
        };
        imp::restore(target)?;
        imp::send_paste(as_)
    }
}

// ───────────────────────────── Windows ─────────────────────────────
#[cfg(windows)]
mod imp {
    use super::{PasteAs, PasteCapability, PasteError, PasteUnsupported};

    pub(super) type Target = isize; // HWND

    const VK_SHIFT: u16 = 0x10;
    const VK_CONTROL: u16 = 0x11;
    const VK_MENU: u16 = 0x12; // Alt
    const VK_LWIN: u16 = 0x5B;
    const VK_RWIN: u16 = 0x5C;
    const VK_V: u16 = 0x56;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const INPUT_KEYBOARD: u32 = 1;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeyBdInput {
        vk: u16,
        scan: u16,
        flags: u32,
        time: u32,
        extra: usize,
    }

    /// `INPUT` — 유니온이 `MOUSEINPUT`(더 크다) 기준이라 꼬리 패딩이 필요하다.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Input {
        kind: u32,
        ki: KeyBdInput,
        _mouse_tail: [u8; 8],
    }

    // ★ 크기가 틀리면 SendInput이 조용히 0을 돌려준다 — 컴파일 시점에 잡는다.
    #[cfg(target_pointer_width = "64")]
    const _: () = assert!(core::mem::size_of::<Input>() == 40);

    // ★ 겹치는 선언(SetForegroundWindow·CreateWindowExW)은 crate::win32 한 곳에만.
    use crate::win32::{CreateWindowExW, SetForegroundWindow};

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn IsWindow(hwnd: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn AttachThreadInput(attach: u32, attach_to: u32, do_attach: i32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
        fn GetAsyncKeyState(vk: i32) -> i16;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    pub(super) fn capability() -> PasteCapability {
        PasteCapability::Full {
            backend: "win32-sendinput",
        }
    }

    pub(super) fn foreground() -> Option<Target> {
        let h = unsafe { GetForegroundWindow() };
        (h != 0).then_some(h)
    }

    pub(super) fn label(t: &Target) -> String {
        let mut buf = [0u16; 128];
        let n = unsafe { GetWindowTextW(*t, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            return format!("hwnd {t:#x}");
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }

    pub(super) fn restore(target: &Target) -> Result<(), PasteError> {
        let hwnd = *target;
        if unsafe { IsWindow(hwnd) } == 0 {
            return Err(PasteError::TargetGone);
        }
        unsafe {
            // ★ AttachThreadInput 없이 SetForegroundWindow만 부르면 깜빡이고 만다.
            let target_tid = GetWindowThreadProcessId(hwnd, core::ptr::null_mut());
            let me = GetCurrentThreadId();
            let attached = target_tid != 0 && target_tid != me;
            if attached {
                AttachThreadInput(me, target_tid, 1);
            }
            let ok = SetForegroundWindow(hwnd);
            if attached {
                AttachThreadInput(me, target_tid, 0);
            }
            if ok == 0 {
                return Err(PasteError::Os("SetForegroundWindow 실패".into()));
            }
        }
        Ok(())
    }

    pub(super) fn send_paste(as_: PasteAs) -> Result<(), PasteError> {
        // ★ 평문도 **Ctrl+V**다(08-31 사용자 실기 "⇧Enter가 팝업만 다시 연다").
        //   재적재가 이미 평문 표현만 올렸으므로 Ctrl+V로 충분하고, Ctrl+Shift+V를 쏘면
        //   그 조합은 **우리 자신의 전역 단축키**(RegisterHotKey)라 대상 앱 대신 우리가
        //   가로채 팝업이 도로 열린다. Linux(포털 RemoteDesktop)도 같은 이유로 Ctrl+V만 쏜다.
        let _ = as_; // 방식 차이는 재적재된 클립보드 내용이 이미 담고 있다(4모드 배선은 T-15b).
        let mut seq: Vec<Input> = Vec::with_capacity(12);
        let key = |vk: u16, up: bool| Input {
            kind: INPUT_KEYBOARD,
            ki: KeyBdInput {
                vk,
                scan: 0,
                flags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                extra: 0,
            },
            _mouse_tail: [0; 8],
        };
        // ★ 사용자가 아직 쥐고 있는 물리 수식 키를 먼저 뗀다(⇧Enter·⇧클릭 직후엔 Shift가,
        //   단축키 직후엔 Ctrl+Shift가 눌린 채다) — 안 떼면 주입한 Ctrl+V와 합쳐져
        //   Ctrl+Shift+V = 우리 전역 단축키가 되거나(팝업 재열림), Win+V(OS 클립보드 기록)가
        //   된다. 끝나면 되눌러 물리 상태와 재동기화한다(다음 실제 keyup이 짝을 찾게).
        let held: Vec<u16> = [VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .filter(|&vk| (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16) & 0x8000 != 0)
            .collect();
        for &vk in &held {
            seq.push(key(vk, true));
        }
        seq.push(key(VK_CONTROL, false));
        seq.push(key(VK_V, false));
        seq.push(key(VK_V, true));
        seq.push(key(VK_CONTROL, true));
        for &vk in &held {
            seq.push(key(vk, false));
        }

        let sent = unsafe {
            SendInput(
                seq.len() as u32,
                seq.as_ptr(),
                core::mem::size_of::<Input>() as i32,
            )
        };
        if sent as usize != seq.len() {
            return Err(PasteError::Os(format!(
                "SendInput {sent}/{} 만 주입됨",
                seq.len()
            )));
        }
        Ok(())
    }

    /// 콘솔 창 핸들 — 있으면 그걸 먼저 쓴다(가장 단순한 경로).
    fn console_window() -> Option<isize> {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetConsoleWindow() -> isize;
        }
        let h = unsafe { GetConsoleWindow() };
        (h != 0).then_some(h)
    }

    /// ★ 스파이크 전용 — **포커스를 실제로 빼앗는다**.
    ///
    /// ⚠️ Windows Terminal 같은 최신 호스트에서는 `GetConsoleWindow`가 0이거나
    /// 그 창을 포그라운드로 못 올린다. 그러면 **탈취가 실패해 대상이 계속 포그라운드로 남고,
    /// 결과적으로 복원 경로(`AttachThreadInput`)가 검증되지 않는다** — 통과처럼 보이는데
    /// 실은 아무것도 안 한 것이다.
    ///
    /// 그래서 콘솔이 안 되면 **임시 최상위 창을 잠깐 띄워** 진짜로 포커스를 가져온다.
    pub(super) fn steal_focus_to_self() -> bool {
        if let Some(h) = console_window() {
            if unsafe { SetForegroundWindow(h) } != 0 {
                return true;
            }
        }
        temp_window_steal()
    }

    /// 임시 최상위 창으로 포커스를 가져온다(만들고 → 올리고 → 지운다).
    fn temp_window_steal() -> bool {
        const WS_EX_TOPMOST: u32 = 0x0000_0008;
        const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
        const WS_POPUP: u32 = 0x8000_0000;
        const SW_SHOW: i32 = 5;

        #[link(name = "user32")]
        extern "system" {
            fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
            fn DestroyWindow(hwnd: isize) -> i32;
        }

        // 미리 등록된 시스템 클래스 "STATIC"을 쓴다 — 클래스 등록·wndproc이 필요 없다.
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title: Vec<u16> = "nexa-clip spike\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                0,
                0,
                0,
                core::ptr::null(),
            )
        };
        if hwnd == 0 {
            return false;
        }
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            let r = SetForegroundWindow(hwnd);
            // 짧게 붙잡았다가 지운다 — 남겨 두면 이 창이 포그라운드를 계속 쥔다.
            std::thread::sleep(std::time::Duration::from_millis(150));
            DestroyWindow(hwnd);
            r != 0
        }
    }

    // 미사용 경고 방지(다른 타깃과 시그니처를 맞추기 위한 항목).
    #[allow(dead_code)]
    fn _unused(_: PasteUnsupported) {}
}

// ───────────────────────────── macOS ─────────────────────────────
#[cfg(target_os = "macos")]
mod imp {
    use super::{PasteAs, PasteCapability, PasteError, PasteUnsupported};

    /// 대상 앱의 PID.
    pub(super) type Target = i32;

    const KVK_ANSI_V: u16 = 0x09;
    const FLAG_COMMAND: u64 = 1 << 20;
    const HID_EVENT_TAP: u32 = 0;

    type CFTypeRef = *mut core::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state: i32) -> CFTypeRef;
        fn CGEventCreateKeyboardEvent(src: CFTypeRef, keycode: u16, keydown: bool) -> CFTypeRef;
        fn CGEventSetFlags(event: CFTypeRef, flags: u64);
        fn CGEventPost(tap: u32, event: CFTypeRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    /// 프런트모스트 앱 PID·활성화는 AppKit이 필요하다. objc 런타임을 직접 부른다
    /// (objc2 의존을 피한다 — DR-8).
    mod appkit {
        use super::CFTypeRef;

        #[link(name = "objc", kind = "dylib")]
        extern "C" {
            fn objc_getClass(name: *const u8) -> CFTypeRef;
            fn sel_registerName(name: *const u8) -> CFTypeRef;
        }
        // AppKit 프레임워크를 링크해야 NSWorkspace 클래스가 로드된다.
        #[link(name = "AppKit", kind = "framework")]
        extern "C" {}

        extern "C" {
            fn objc_msgSend();
        }

        // ⚠️ 크레이트가 `forbid(unsafe_op_in_unsafe_fn)` — `unsafe fn` 안에서도 블록이 필요하다.
        unsafe fn cls(name: &[u8]) -> CFTypeRef {
            unsafe { objc_getClass(name.as_ptr()) }
        }
        unsafe fn sel(name: &[u8]) -> CFTypeRef {
            unsafe { sel_registerName(name.as_ptr()) }
        }

        unsafe fn send0(recv: CFTypeRef, s: &[u8]) -> CFTypeRef {
            unsafe {
                let f: unsafe extern "C" fn(CFTypeRef, CFTypeRef) -> CFTypeRef =
                    core::mem::transmute(objc_msgSend as *const ());
                f(recv, sel(s))
            }
        }

        unsafe fn send0_i32(recv: CFTypeRef, s: &[u8]) -> i32 {
            unsafe {
                let f: unsafe extern "C" fn(CFTypeRef, CFTypeRef) -> i32 =
                    core::mem::transmute(objc_msgSend as *const ());
                f(recv, sel(s))
            }
        }

        unsafe fn send1_bool(recv: CFTypeRef, s: &[u8], arg: u64) -> bool {
            unsafe {
                let f: unsafe extern "C" fn(CFTypeRef, CFTypeRef, u64) -> bool =
                    core::mem::transmute(objc_msgSend as *const ());
                f(recv, sel(s), arg)
            }
        }

        /// 지금 프런트모스트 앱의 PID.
        pub(super) fn frontmost_pid() -> Option<i32> {
            unsafe {
                let ws = send0(cls(b"NSWorkspace\0"), b"sharedWorkspace\0");
                if ws.is_null() {
                    return None;
                }
                let app = send0(ws, b"frontmostApplication\0");
                if app.is_null() {
                    return None;
                }
                let pid = send0_i32(app, b"processIdentifier\0");
                (pid > 0).then_some(pid)
            }
        }

        /// PID로 앱을 활성화한다(`NSRunningApplication` · `activateAllWindows`).
        pub(super) fn activate_pid(pid: i32) -> bool {
            unsafe {
                let f: unsafe extern "C" fn(CFTypeRef, CFTypeRef, i32) -> CFTypeRef =
                    core::mem::transmute(objc_msgSend as *const ());
                let app = f(
                    cls(b"NSRunningApplication\0"),
                    sel(b"runningApplicationWithProcessIdentifier:\0"),
                    pid,
                );
                if app.is_null() {
                    return false;
                }
                // NSApplicationActivateAllWindows | IgnoringOtherApps
                send1_bool(app, b"activateWithOptions:\0", 0b11)
            }
        }
    }

    pub(super) fn capability() -> PasteCapability {
        if unsafe { AXIsProcessTrusted() } {
            PasteCapability::Full {
                backend: "mac-cgevent",
            }
        } else {
            PasteCapability::NeedsPermission {
                backend: "mac-cgevent",
                hint:
                    "시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용에서 Nexa Clip을 허용하세요",
            }
        }
    }

    pub(super) fn foreground() -> Option<Target> {
        appkit::frontmost_pid()
    }

    pub(super) fn label(t: &Target) -> String {
        format!("pid {t}")
    }

    pub(super) fn restore(target: &Target) -> Result<(), PasteError> {
        if appkit::activate_pid(*target) {
            Ok(())
        } else {
            Err(PasteError::TargetGone)
        }
    }

    pub(super) fn send_paste(as_: PasteAs) -> Result<(), PasteError> {
        if !unsafe { AXIsProcessTrusted() } {
            return Err(PasteError::PermissionDenied {
                hint: "시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용",
            });
        }
        // ★ 평문도 **⌘V**다(09-04 mac 실기 "⇧Enter·⇧⌥X 미동작" — Windows 08-31과 같은 결론):
        //   재적재가 이미 평문 표현만 올렸으므로 ⌘V로 충분하고, ⌘⇧V는 앱마다 다른 기능
        //   (TextEdit "스타일 일치"는 ⌥⇧⌘V · 터미널은 없음)이라 어디서도 보장되지 않는다.
        //   방식 차이는 클립보드 내용이 담는다(T-15b · 4모드 = 내용 선별). 플래그를 명시로
        //   박으므로 사용자가 아직 쥔 물리 ⇧/⌥(⇧Enter · ⇧⌥X 직후)는 합성 이벤트에 섞이지 않는다.
        let _ = as_;
        unsafe {
            let src = CGEventSourceCreate(1); // kCGEventSourceStateHIDSystemState
            for down in [true, false] {
                let ev = CGEventCreateKeyboardEvent(src, KVK_ANSI_V, down);
                if ev.is_null() {
                    return Err(PasteError::Os("CGEvent 생성 실패".into()));
                }
                CGEventSetFlags(ev, FLAG_COMMAND);
                CGEventPost(HID_EVENT_TAP, ev);
                CFRelease(ev);
            }
            if !src.is_null() {
                CFRelease(src);
            }
        }
        Ok(())
    }

    /// 스파이크 전용 — 우리 프로세스가 포커스를 가져간다.
    pub(super) fn steal_focus_to_self() -> bool {
        appkit::activate_pid(std::process::id() as i32)
    }

    #[allow(dead_code)]
    fn _unused(_: PasteUnsupported) {}
}

// ───────────────────────────── 그 외(Linux 등) ─────────────────────────────
// ───────────────────────────── Linux ─────────────────────────────
/// Linux — X11 `XTest` 키 주입(x11rb · 순수 Rust · 원장 docs/10 §3).
///
/// 두 세션이 갈린다:
/// - **X11 세션**: `GetInputFocus`로 대상 창을 기억 → 팝업이 닫힌 뒤 `_NET_ACTIVE_WINDOW`
///   클라이언트 메시지(EWMH — WM이 포커스·스택 정리) + `SetInputFocus` → XTest `Ctrl+V`.
/// - **Wayland 세션**: 클라이언트는 남의 창을 활성화할 수 없다(xdg-shell 한계). 대신
///   **팝업이 닫히면 컴포지터가 직전 창에 포커스를 돌려준다** — `restore`는 그 정착만
///   기다린다. 주입은 **XWayland의 XTest**(`DISPLAY=:0`) — Mutter는 XTest를 가상 입력
///   장치로 받아 Wayland 네이티브 앱에도 배달한다. `DISPLAY`가 없으면(순수 Wayland) 표준이
///   없으므로 `WaylandNoInjection`.
///
/// 평문(`PasteAs::Plain`)도 `Ctrl+V` — 호스트가 클립보드에 평문 표현만 올린다(Windows
/// 어댑터의 `Ctrl+Shift+V` 관례는 Linux에선 우리 전역 단축키와 겹쳐 쓰지 않는다).
#[cfg(target_os = "linux")]
mod imp {
    use super::{PasteAs, PasteCapability, PasteError, PasteUnsupported};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{
        self, ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus,
    };
    use x11rb::protocol::xtest::ConnectionExt as _;
    use x11rb::rust_connection::RustConnection;

    /// 붙여넣기 대상.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) enum Target {
        /// X11 세션 — 포커스 창 id.
        X11 { window: u32 },
        /// Wayland 세션 — 컴포지터가 포커스를 돌려준다(창 id를 알 수 없다).
        Wayland,
    }

    const KEYSYM_V: u32 = 0x76;
    const KEYSYM_CONTROL_L: u32 = 0xffe3;
    const KEY_PRESS: u8 = xproto::KEY_PRESS_EVENT;
    const KEY_RELEASE: u8 = xproto::KEY_RELEASE_EVENT;
    /// Wayland 컴포지터의 포커스 반환·X11 WM의 활성화가 정착할 시간.
    const SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

    fn is_wayland() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
    }

    fn has_display() -> bool {
        std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty())
    }

    fn connect() -> Result<(RustConnection, usize), PasteError> {
        RustConnection::connect(None).map_err(|e| PasteError::Os(format!("X 연결 실패: {e}")))
    }

    pub(super) fn capability() -> PasteCapability {
        if !has_display() && !(is_wayland() && crate::remote_input_linux::available()) {
            return PasteCapability::ClipboardOnly {
                reason: if is_wayland() {
                    PasteUnsupported::WaylandNoInjection
                } else {
                    PasteUnsupported::NoDisplayServer
                },
            };
        }
        // ★ Wayland 세션은 포털 RemoteDesktop이 정식(08-30 실기: ei-portal Xwayland에선
        //   XTest가 앱까지 못 간다). 포털이 없을 때만 XWayland XTest로.
        if is_wayland() && crate::remote_input_linux::available() {
            return PasteCapability::Full {
                backend: "wayland-portal-remotedesktop",
            };
        }
        let Ok((conn, _)) = connect() else {
            return PasteCapability::ClipboardOnly {
                reason: PasteUnsupported::NoDisplayServer,
            };
        };
        let has_xtest = conn
            .xtest_get_version(2, 2)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some();
        if !has_xtest {
            return PasteCapability::ClipboardOnly {
                reason: PasteUnsupported::NotImplemented,
            };
        }
        PasteCapability::Full {
            backend: if is_wayland() {
                "xwayland-xtest"
            } else {
                "x11-xtest"
            },
        }
    }

    pub(super) fn foreground() -> Option<Target> {
        if is_wayland() && crate::remote_input_linux::available() {
            return Some(Target::Wayland);
        }
        if !has_display() {
            return None;
        }
        if is_wayland() {
            return Some(Target::Wayland);
        }
        let (conn, _) = connect().ok()?;
        let focus = conn.get_input_focus().ok()?.reply().ok()?.focus;
        // None(0)·PointerRoot(1)은 창이 아니다.
        (focus > 1).then_some(Target::X11 { window: focus })
    }

    pub(super) fn label(t: &Target) -> String {
        match t {
            Target::X11 { window } => format!("X11 창 0x{window:x}"),
            Target::Wayland => "(Wayland — 컴포지터가 직전 창으로 되돌림)".into(),
        }
    }

    pub(super) fn restore(t: &Target) -> Result<(), PasteError> {
        match t {
            Target::Wayland => {
                std::thread::sleep(SETTLE);
                Ok(())
            }
            Target::X11 { window } => {
                let (conn, screen) = connect()?;
                let root = conn
                    .setup()
                    .roots
                    .get(screen)
                    .ok_or_else(|| PasteError::Os("X 화면 없음".into()))?
                    .root;
                // 창이 아직 있는가.
                if conn
                    .get_window_attributes(*window)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .is_none()
                {
                    return Err(PasteError::TargetGone);
                }
                // EWMH `_NET_ACTIVE_WINDOW`(source = 2: 페이저/도구) — WM이 있으면 이것이 정식.
                let atom = conn
                    .intern_atom(false, b"_NET_ACTIVE_WINDOW")
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|r| r.atom);
                if let Some(atom) = atom {
                    let ev = ClientMessageEvent::new(32, *window, atom, [2, 0, 0, 0, 0]);
                    let _ = conn.send_event(
                        false,
                        root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        ev,
                    );
                }
                // WM이 없을 때의 폴백(둘 다 보내도 무해).
                let _ =
                    conn.set_input_focus(InputFocus::POINTER_ROOT, *window, x11rb::CURRENT_TIME);
                conn.flush()
                    .map_err(|e| PasteError::Os(format!("X flush 실패: {e}")))?;
                std::thread::sleep(SETTLE);
                Ok(())
            }
        }
    }

    pub(super) fn send_paste(_as: PasteAs) -> Result<(), PasteError> {
        if is_wayland() && crate::remote_input_linux::available() {
            return crate::remote_input_linux::tap_ctrl_v().map_err(PasteError::Os);
        }
        if !has_display() {
            return Err(PasteError::Unsupported(if is_wayland() {
                PasteUnsupported::WaylandNoInjection
            } else {
                PasteUnsupported::NoDisplayServer
            }));
        }
        let (conn, screen) = connect()?;
        let setup = conn.setup();
        let root = setup
            .roots
            .get(screen)
            .ok_or_else(|| PasteError::Os("X 화면 없음".into()))?
            .root;
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let map = conn
            .get_keyboard_mapping(min, max - min + 1)
            .map(|c| c.reply())
            .map_err(|e| PasteError::Os(format!("키맵 조회 실패: {e}")))?
            .map_err(|e| PasteError::Os(format!("키맵 응답 실패: {e}")))?;
        let per = usize::from(map.keysyms_per_keycode);
        let kc = |sym: u32| {
            find_keycode(&map.keysyms, per, min, sym)
                .ok_or_else(|| PasteError::Os(format!("keysym 0x{sym:x}의 keycode 없음")))
        };
        let ctrl = kc(KEYSYM_CONTROL_L)?;
        let v = kc(KEYSYM_V)?;
        let fake = |ty: u8, code: u8| {
            conn.xtest_fake_input(ty, code, x11rb::CURRENT_TIME, root, 0, 0, 0)
                .map_err(|e| PasteError::Os(format!("XTest 실패: {e}")))
                .map(|_| ())
        };
        fake(KEY_PRESS, ctrl)?;
        fake(KEY_PRESS, v)?;
        fake(KEY_RELEASE, v)?;
        fake(KEY_RELEASE, ctrl)?;
        conn.flush()
            .map_err(|e| PasteError::Os(format!("X flush 실패: {e}")))?;
        // 서버가 처리했는지 왕복 한 번(에러 이벤트는 여기서 드러난다).
        conn.get_input_focus()
            .map(|c| c.reply())
            .map_err(|e| PasteError::Os(format!("X 왕복 실패: {e}")))?
            .map_err(|e| PasteError::Os(format!("X 왕복 응답 실패: {e}")))?;
        Ok(())
    }

    /// keysym → keycode — `GetKeyboardMapping` 표(keycode마다 `per`개 keysym)에서 첫 일치.
    pub(super) fn find_keycode(keysyms: &[u32], per: usize, min: u8, sym: u32) -> Option<u8> {
        if per == 0 {
            return None;
        }
        keysyms
            .chunks(per)
            .position(|row| row.contains(&sym))
            .and_then(|i| u8::try_from(i).ok())
            .and_then(|i| min.checked_add(i))
    }

    pub(super) fn steal_focus_to_self() -> bool {
        false // 창 없는 스파이크에서 흉내 낼 방법이 없다(Wayland) — 실물 팝업으로 검증.
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// ★ X11 경로 실기(자동) — **자기 창을 만들어 포커스를 잡고** XTest로 `Ctrl+V`를
        /// 넣은 뒤 그 창이 KeyPress(Control_L · v)를 받는지 본다. Xvfb에서 사람 없이 돈다:
        /// `Xvfb :99 & DISPLAY=:99 cargo test -p nclip-plat -- --ignored x11_xtest`
        /// (XWayland `:0`에서는 컴포지터가 포커스를 주지 않아 KeyPress가 안 온다 — 그건
        /// 사람이 실제 앱으로 본다 · docs/21 §2-5).
        #[test]
        #[ignore = "X 서버가 필요(Xvfb 수동 실행 전용)"]
        fn x11_xtest_roundtrip_into_own_window() {
            use x11rb::protocol::xproto::{CreateWindowAux, WindowClass};
            use x11rb::protocol::Event;
            let (conn, screen) = connect().expect("X 연결");
            let root_info = conn.setup().roots[screen].clone();
            let win = conn.generate_id().expect("id");
            conn.create_window(
                0,
                win,
                root_info.root,
                0,
                0,
                100,
                100,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new().event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE),
            )
            .expect("create")
            .check()
            .expect("create ok");
            conn.map_window(win).expect("map");
            conn.set_input_focus(InputFocus::POINTER_ROOT, win, x11rb::CURRENT_TIME)
                .expect("focus");
            conn.flush().expect("flush");
            std::thread::sleep(std::time::Duration::from_millis(200));
            // 대상 기억 → 복원 → 주입(어댑터 공개 경로 그대로).
            let target = foreground().expect("포커스 창");
            assert_eq!(target, Target::X11 { window: win });
            restore(&target).expect("복원");
            send_paste(PasteAs::Original).expect("주입");
            // 우리 창에 Control_L·v KeyPress가 도착해야 한다.
            let mut got = Vec::new();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while std::time::Instant::now() < deadline && got.len() < 2 {
                if let Ok(Some(Event::KeyPress(k))) = conn.poll_for_event() {
                    got.push(k.detail);
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
            let setup = conn.setup();
            let (min, max) = (setup.min_keycode, setup.max_keycode);
            let map = conn
                .get_keyboard_mapping(min, max - min + 1)
                .expect("map")
                .reply()
                .expect("map reply");
            let per = usize::from(map.keysyms_per_keycode);
            let ctrl = find_keycode(&map.keysyms, per, min, KEYSYM_CONTROL_L).expect("ctrl");
            let v = find_keycode(&map.keysyms, per, min, KEYSYM_V).expect("v");
            assert_eq!(got, vec![ctrl, v], "Ctrl 다음 V 순서로 눌려야 한다");
        }

        /// keycode 표 해석 — 행 폭 `per`, 첫 일치 행의 keycode = min + 행 번호.
        #[test]
        fn keycode_lookup_reads_rows() {
            // min=8 · per=2 · keycode 8=[a,A] 9=[v,V] 10=[Control_L,0]
            let syms = [0x61, 0x41, 0x76, 0x56, 0xffe3, 0];
            assert_eq!(find_keycode(&syms, 2, 8, 0x76), Some(9));
            assert_eq!(find_keycode(&syms, 2, 8, 0xffe3), Some(10));
            assert_eq!(find_keycode(&syms, 2, 8, 0x7a), None);
            assert_eq!(find_keycode(&syms, 0, 8, 0x76), None);
        }
    }
}

// ───────────────────────────── 기타 ─────────────────────────────
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod imp {
    use super::{PasteAs, PasteCapability, PasteError, PasteUnsupported};

    pub(super) type Target = ();

    pub(super) fn capability() -> PasteCapability {
        PasteCapability::ClipboardOnly {
            reason: PasteUnsupported::NotImplemented,
        }
    }

    pub(super) fn foreground() -> Option<Target> {
        None
    }

    pub(super) fn label(_: &Target) -> String {
        "(미구현)".into()
    }

    pub(super) fn restore(_: &Target) -> Result<(), PasteError> {
        Err(PasteError::Unsupported(PasteUnsupported::NotImplemented))
    }

    pub(super) fn send_paste(_: PasteAs) -> Result<(), PasteError> {
        Err(PasteError::Unsupported(PasteUnsupported::NotImplemented))
    }

    pub(super) fn steal_focus_to_self() -> bool {
        false
    }
}

/// Linux/Wayland — 포털 `RemoteDesktop` 세션을 **미리** 연다(첫 회 권한 대화창을 시작 때
/// 받기 위해 · `token_path` = `restore_token` 보관 파일). 다른 OS·X11 = no-op.
pub fn warm_up(token_path: Option<std::path::PathBuf>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && crate::remote_input_linux::available() {
            if let Some(p) = token_path {
                crate::remote_input_linux::configure_token_path(p);
            }
            return crate::remote_input_linux::ensure_session();
        }
    }
    let _ = token_path;
    Ok(())
}

/// 스파이크 전용 — 팝업이 포커스를 뺏는 순간을 흉내 낸다.
///
/// 실물에서는 창이 뜨면서 자연히 일어나는 일이라, 창 없이 그 왕복을 검증하려고 둔다.
#[must_use]
/// ★ 창을 진짜 포그라운드로(09-01 사용자 실기 "트레이 클릭 시 창이 뒤로 숨음") —
/// Windows는 포그라운드 권한 규칙 때문에 `focus_window()`만으로는 작업표시줄만
/// 깜박인다. K-1 복원과 같은 AttachThreadInput 문법을 재사용한다.
/// 다른 OS는 no-op(컴포지터/WM이 알아서 — Linux는 wlactivate 경로가 담당).
pub fn force_foreground(hwnd: isize) -> bool {
    #[cfg(windows)]
    {
        imp::restore(&hwnd).is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        false
    }
}

pub fn spike_steal_focus() -> bool {
    imp::steal_focus_to_self()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 대상을 기억하지 않고 붙여넣기를 시도하면 **조용히 성공하지 않는다**.
    #[test]
    fn paste_without_capture_fails_loudly() {
        let mut p = PlatformPaste::new();
        assert_eq!(
            p.restore_and_paste(PasteAs::Original),
            Err(PasteError::TargetGone)
        );
    }

    /// capability는 언제나 답을 준다(패닉·무응답 없음) — 온보딩 점검이 여기 의존한다.
    #[test]
    fn capability_answers() {
        let p = PlatformPaste::new();
        let c = p.capability();
        // 어느 변형이든 진단 가능한 정보를 담는다.
        match c {
            PasteCapability::Full { backend } => assert!(!backend.is_empty()),
            PasteCapability::NeedsPermission { backend, hint } => {
                assert!(!backend.is_empty());
                assert!(!hint.is_empty(), "권한 안내가 비어 있으면 사용자가 막힌다");
            }
            PasteCapability::ClipboardOnly { .. } => {}
        }
    }
}
