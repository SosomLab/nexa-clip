//! 콘솔 종료 신호(Ctrl+C·창 닫기) — ★ **상주 앱의 정상 종료 경로로 바꾼다**.
//!
//! `cargo run -- tray`를 Ctrl+C로 끊으면 프로세스가 `STATUS_CONTROL_C_EXIT`로 죽고
//! cargo가 **오류처럼** 찍는다(08-28 실기 — 사용자가 오류로 오인). 핸들러를 걸어
//! 신호를 셸의 종료 이벤트로 돌리면 설정 flush까지 거친 **exit 0**이 된다.

use std::sync::OnceLock;

static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// 콘솔 종료 신호(Ctrl+C·Ctrl+Break·창 닫기·로그오프)에 `f`를 부른다.
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

#[cfg(not(windows))]
mod imp {
    /// 미이식 타깃 — 시그널 처리는 그 OS 작업에서(정직하게 false).
    pub(super) fn install() -> bool {
        false
    }
}
