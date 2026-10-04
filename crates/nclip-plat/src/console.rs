//! 콘솔 종료 신호(Ctrl+C·창 닫기) — ★ **상주 앱의 정상 종료 경로로 바꾼다**.
//!
//! `cargo run -- tray`를 Ctrl+C로 끊으면 프로세스가 `STATUS_CONTROL_C_EXIT`로 죽고
//! cargo가 **오류처럼** 찍는다(08-28 실기 — 사용자가 오류로 오인). 핸들러를 걸어
//! 신호를 셸의 종료 이벤트로 돌리면 설정 flush까지 거친 **exit 0**이 된다.

use std::sync::OnceLock;

/// ★ 부모 콘솔 붙기(09-03) — 본체가 `windows` 서브시스템(콘솔 창 없음)이 되면
/// 터미널에서 실행해도 출력이 사라진다. 터미널이 부모면 그 콘솔을 붙여
/// 진단 출력(status·peek·감시 로그)을 살린다. 더블클릭이면 조용히 실패(창도 콘솔도 없음).
pub fn attach_parent() {
    #[cfg(windows)]
    // SAFETY: 인자 없는 단순 호출 — 실패는 무시한다.
    unsafe {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(pid: u32) -> i32;
        }
        AttachConsole(u32::MAX); // ATTACH_PARENT_PROCESS
    }
}

static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// 콘솔 종료 신호(Windows: Ctrl+C·Ctrl+Break·창 닫기·로그오프 · Unix: `SIGINT`·`SIGTERM`)에 `f`를 부른다.
/// 프로세스당 1회 — 성공 여부 반환(미지원 타깃·재호출은 `false`).
pub fn on_console_quit<F: Fn() + Send + Sync + 'static>(f: F) -> bool {
    if HANDLER.set(Box::new(f)).is_err() {
        return false;
    }
    imp::install()
}

#[cfg(windows)]
mod imp {
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }

    /// CTRL_C(0)·CTRL_BREAK(1)·CTRL_CLOSE(2)·LOGOFF(5)·SHUTDOWN(6).
    ///
    /// TRUE(1)를 돌려 기본 강제 종료를 막고, 등록된 콜백이 셸에 종료를 요청한다.
    /// CLOSE/LOGOFF/SHUTDOWN은 시스템이 유예 후 어차피 끝내지만, 그 유예 동안
    /// 설정 flush가 돌 기회를 얻는다.
    unsafe extern "system" fn handler(ev: u32) -> i32 {
        if matches!(ev, 0 | 1 | 2 | 5 | 6) {
            if let Some(f) = super::HANDLER.get() {
                f();
            }
            1
        } else {
            0
        }
    }

    pub(super) fn install() -> bool {
        // SAFETY: 정적 핸들러 등록 — 콜백은 위 handler 하나뿐이다.
        unsafe { SetConsoleCtrlHandler(Some(handler), 1) != 0 }
    }
}

/// ★ Unix(Linux·mac) — `SIGINT`(Ctrl+C)·`SIGTERM`(`kill`·세션 종료·재시작 스크립트)을 정상 종료로(10-05 · T-41 ⑦).
///
/// 시그널 핸들러 안에서는 할 수 있는 일이 거의 없다(async-signal-safe) — 파이프에 한 바이트만 쓰고,
/// 전용 스레드가 그것을 읽어 콜백을 부른다(self-pipe). 외부 crate 없이 libc 심볼만 쓴다(DR-8).
#[cfg(unix)]
mod imp {
    use std::sync::atomic::{AtomicI32, Ordering};

    extern "C" {
        fn pipe(fds: *mut i32) -> i32;
        fn signal(sig: i32, handler: usize) -> usize;
        fn write(fd: i32, buf: *const u8, n: usize) -> isize;
        fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    }
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    /// 파이프의 쓰는 쪽 fd — 핸들러가 읽는다(원자 정수만 만진다).
    static WR: AtomicI32 = AtomicI32::new(-1);

    extern "C" fn on_signal(_sig: i32) {
        let fd = WR.load(Ordering::Relaxed);
        if fd >= 0 {
            let b = 1u8;
            // SAFETY: write(2)는 async-signal-safe다 — 유효한 fd에 1바이트.
            unsafe {
                write(fd, &b, 1);
            }
        }
    }

    pub(super) fn install() -> bool {
        let mut fds = [0i32; 2];
        // SAFETY: 길이 2 배열에 fd 두 개를 받는다.
        if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
            return false;
        }
        let rd = fds[0];
        WR.store(fds[1], Ordering::Relaxed);
        let spawned = std::thread::Builder::new()
            .name("nclip-signal".into())
            .spawn(move || loop {
                let mut b = 0u8;
                // SAFETY: 유효한 fd에서 1바이트 읽기(블로킹).
                let n = unsafe { read(rd, &mut b, 1) };
                if n == 1 {
                    if let Some(h) = super::HANDLER.get() {
                        h();
                    }
                } else if n == 0 {
                    return;
                } else {
                    // EINTR 등 — 잠깐 쉬고 다시(바쁜 루프 방지).
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            });
        if spawned.is_err() {
            return false;
        }
        // SAFETY: 핸들러는 위의 async-signal-safe 함수 하나다.
        unsafe {
            signal(SIGINT, on_signal as *const () as usize);
            signal(SIGTERM, on_signal as *const () as usize);
        }
        true
    }
}

#[cfg(not(any(windows, unix)))]
mod imp {
    /// 미이식 타깃 — 정직하게 false.
    pub(super) fn install() -> bool {
        false
    }
}
