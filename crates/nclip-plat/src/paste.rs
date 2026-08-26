//! 포커스 복원 + 키 주입 어댑터 — ★ **K-1 스파이크**([docs/02 §7](../../../docs/02-roadmap.md)).
//!
//! 외부 crate를 쓰지 않는다(DR-8) — OS 함수를 `#[link]` + `extern`으로 직접 선언한다.
//!
//! | OS | 상태 | 구현 |
//! |---|---|---|
//! | **Windows** | ✅ 구현 | `GetForegroundWindow` → `AttachThreadInput` + `SetForegroundWindow` → `SendInput` |
//! | **macOS** | 🚧 구현(실기 미검증) | `CGEventPost`(⌘V) + `AXIsProcessTrusted` 권한 확인 |
//! | **Linux** | ✕ 미구현 | X11 `XTestFakeKeyEvent` 예정 · **Wayland는 표준 없음** |
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

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn IsWindow(hwnd: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn AttachThreadInput(attach: u32, attach_to: u32, do_attach: i32) -> i32;
        fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
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
        // 원본 = Ctrl+V · 평문 = Ctrl+Shift+V(대상 앱의 관례를 빌린다).
        // ⚠️ 평문은 앱마다 지원이 갈린다 — 확실한 길은 "평문만 클립보드에 올리고 Ctrl+V"다.
        let plain = matches!(as_, PasteAs::Plain);
        let mut seq: Vec<Input> = Vec::with_capacity(6);
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
        seq.push(key(VK_CONTROL, false));
        if plain {
            seq.push(key(VK_SHIFT, false));
        }
        seq.push(key(VK_V, false));
        seq.push(key(VK_V, true));
        if plain {
            seq.push(key(VK_SHIFT, true));
        }
        seq.push(key(VK_CONTROL, true));

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
            fn CreateWindowExW(
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
            fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
            fn DestroyWindow(hwnd: isize) -> i32;
        }

        // 미리 등록된 시스템 클래스 "STATIC"을 쓴다 — 클래스 등록·wndproc이 필요 없다.
        let class: Vec<u16> = "STATIC ".encode_utf16().collect();
        let title: Vec<u16> = "nexa-clip spike ".encode_utf16().collect();
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
    const KVK_SHIFT: u16 = 0x38;
    const FLAG_COMMAND: u64 = 1 << 20;
    const FLAG_SHIFT: u64 = 1 << 17;
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
        let plain = matches!(as_, PasteAs::Plain);
        let flags = FLAG_COMMAND | if plain { FLAG_SHIFT } else { 0 };
        unsafe {
            let src = CGEventSourceCreate(1); // kCGEventSourceStateHIDSystemState
            for down in [true, false] {
                if plain {
                    let ev = CGEventCreateKeyboardEvent(src, KVK_SHIFT, down);
                    if ev.is_null() {
                        return Err(PasteError::Os("CGEvent 생성 실패".into()));
                    }
                    CGEventSetFlags(ev, flags);
                    CGEventPost(HID_EVENT_TAP, ev);
                    CFRelease(ev);
                }
                let ev = CGEventCreateKeyboardEvent(src, KVK_ANSI_V, down);
                if ev.is_null() {
                    return Err(PasteError::Os("CGEvent 생성 실패".into()));
                }
                CGEventSetFlags(ev, flags);
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
#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::{PasteAs, PasteCapability, PasteError, PasteUnsupported};

    pub(super) type Target = ();

    pub(super) fn capability() -> PasteCapability {
        // X11 XTest는 T-15b, Wayland는 구조적으로 표준이 없다.
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

/// 스파이크 전용 — 팝업이 포커스를 뺏는 순간을 흉내 낸다.
///
/// 실물에서는 창이 뜨면서 자연히 일어나는 일이라, 창 없이 그 왕복을 검증하려고 둔다.
#[must_use]
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
