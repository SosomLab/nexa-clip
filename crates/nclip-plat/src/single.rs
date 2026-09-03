//! ★ T-12e4 — **단일 인스턴스 가드**(3-OS · 09-03).
//!
//! 자동 시작 상주 중 런처·터미널에서 재실행하면 감시 2중·트레이 아이콘 2개가
//! 된다(09-03 관찰). 둘째 실행은 **기존 인스턴스에 "열기"를 위임**하고 종료한다.
//!
//! - Windows: 이름 있는 뮤텍스로 판정 + 이름 있는 이벤트로 "열기" 신호
//!   (첫 인스턴스는 대기 스레드가 이벤트를 받아 콜백을 부른다).
//! - Unix: `data/` 아래 잠금 파일 `flock`(비블로킹) — 위임 신호는 후속
//!   (지금은 안내 후 종료 · 잠금 자체가 2중 상주를 막는다).

/// 살아 있는 동안 인스턴스 소유권을 지키는 가드 — 프로세스 수명만큼 들고 있어야 한다.
#[derive(Debug)]
pub struct InstanceGuard {
    #[cfg(windows)]
    _mutex: isize,
    #[cfg(unix)]
    _file: std::fs::File,
}

#[cfg(windows)]
mod win {
    pub(super) type Handle = isize;
    #[link(name = "kernel32")]
    extern "system" {
        pub(super) fn CreateMutexW(attrs: *const u8, own: i32, name: *const u16) -> Handle;
        pub(super) fn GetLastError() -> u32;
        pub(super) fn CreateEventW(
            attrs: *const u8,
            manual: i32,
            initial: i32,
            name: *const u16,
        ) -> Handle;
        pub(super) fn SetEvent(h: Handle) -> i32;
        pub(super) fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
        pub(super) fn CloseHandle(h: Handle) -> i32;
    }
    pub(super) const ERROR_ALREADY_EXISTS: u32 = 183;
    pub(super) const INFINITE: u32 = 0xFFFF_FFFF;

    pub(super) fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(core::iter::once(0)).collect()
    }
}

/// 뮤텍스/잠금 이름 — 사용자 세션 단위(전역 아님: 다중 사용자 PC 배려).
#[cfg(windows)]
const MUTEX_NAME: &str = "Local\\NexaClip.SingleInstance";
#[cfg(windows)]
const OPEN_EVENT_NAME: &str = "Local\\NexaClip.OpenRequest";

/// 인스턴스 소유를 시도한다.
///
/// - `Some(guard)` = 우리가 첫 인스턴스 — 가드를 프로세스 수명만큼 유지할 것.
/// - `None` = 이미 상주 중 — [`signal_open`]으로 위임하고 종료하라.
///
/// `lock_path`는 Unix 잠금 파일 위치(Windows에선 무시).
#[must_use]
pub fn acquire(lock_path: &std::path::Path) -> Option<InstanceGuard> {
    #[cfg(windows)]
    {
        let _ = lock_path;
        // SAFETY: 실패는 널/0으로 돌아오고 그때마다 빠져나간다.
        unsafe {
            let h = win::CreateMutexW(core::ptr::null(), 0, win::wide(MUTEX_NAME).as_ptr());
            if h == 0 {
                // 뮤텍스조차 못 만들면 가드 없이 진행(안 뜨는 것보다 낫다 · DR-31).
                return Some(InstanceGuard { _mutex: 0 });
            }
            if win::GetLastError() == win::ERROR_ALREADY_EXISTS {
                win::CloseHandle(h);
                return None;
            }
            Some(InstanceGuard { _mutex: h })
        }
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd as _;
        if let Some(dir) = lock_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
            .ok()?;
        // SAFETY: 유효한 fd에 대한 비블로킹 flock — 실패 = 이미 잠김.
        let rc = unsafe { libc_flock(file.as_raw_fd(), 2 | 4) }; // LOCK_EX | LOCK_NB
        if rc != 0 {
            return None;
        }
        Some(InstanceGuard { _file: file })
    }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "flock"]
    fn libc_flock(fd: i32, op: i32) -> i32;
}

/// 둘째 실행 — 기존 인스턴스에 "열기"를 알린다(Windows · Unix는 후속).
pub fn signal_open() {
    #[cfg(windows)]
    // SAFETY: 이름으로 열기 실패 = 0 → SetEvent가 그냥 실패한다.
    unsafe {
        let h = win::CreateEventW(core::ptr::null(), 0, 0, win::wide(OPEN_EVENT_NAME).as_ptr());
        if h != 0 {
            win::SetEvent(h);
            win::CloseHandle(h);
        }
    }
}

/// 첫 인스턴스 — "열기" 신호를 기다렸다가 `on_open`을 부른다(백그라운드 스레드).
pub fn watch_open_requests(on_open: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    {
        std::thread::Builder::new()
            .name("nclip-single".into())
            .spawn(move || {
                // SAFETY: 자동 리셋 이벤트를 만들어(이미 있으면 그 핸들) 무한 대기 루프.
                unsafe {
                    let h = win::CreateEventW(
                        core::ptr::null(),
                        0,
                        0,
                        win::wide(OPEN_EVENT_NAME).as_ptr(),
                    );
                    if h == 0 {
                        return;
                    }
                    loop {
                        if win::WaitForSingleObject(h, win::INFINITE) != 0 {
                            return;
                        }
                        on_open();
                    }
                }
            })
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = on_open;
    }
}
