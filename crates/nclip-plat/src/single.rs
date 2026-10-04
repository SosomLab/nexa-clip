//! ★ T-12e4 — **단일 인스턴스 가드**(3-OS · 09-03).
//!
//! 자동 시작 상주 중 런처·터미널에서 재실행하면 감시 2중·트레이 아이콘 2개가
//! 된다(09-03 관찰). 둘째 실행은 **기존 인스턴스에 "열기"를 위임**하고 종료한다.
//!
//! - Windows: 이름 있는 뮤텍스로 판정 + 이름 있는 이벤트로 "열기" 신호
//!   (첫 인스턴스는 대기 스레드가 이벤트를 받아 콜백을 부른다).
//! - Unix: `data/` 아래 잠금 파일 `flock`(비블로킹) + ★ 같은 폴더의 **Unix 소켓**으로 "열기" 위임
//!   (10-05 · T-41 ⑧ — 첫 인스턴스가 듣고, 둘째 실행이 접속해 한 줄 쓴다 · std만 쓴다).

/// 살아 있는 동안 인스턴스 소유권을 지키는 가드 — 프로세스 수명만큼 들고 있어야 한다.
#[derive(Debug)]
pub struct InstanceGuard {
    #[cfg(windows)]
    _mutex: isize,
    #[cfg(unix)]
    _file: Option<std::fs::File>,
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
/// ★ `tag`(프로필 · 09-04)가 있으면 이름 뒤에 붙어 **프로필마다 별개 인스턴스**가 된다.
#[cfg(windows)]
fn mutex_name(tag: Option<&str>) -> String {
    match tag {
        Some(t) => format!("Local\\NexaClip.SingleInstance.{t}"),
        None => "Local\\NexaClip.SingleInstance".to_string(),
    }
}
#[cfg(windows)]
fn open_event_name(tag: Option<&str>) -> String {
    match tag {
        Some(t) => format!("Local\\NexaClip.OpenRequest.{t}"),
        None => "Local\\NexaClip.OpenRequest".to_string(),
    }
}

/// 인스턴스 소유를 시도한다.
///
/// - `Some(guard)` = 우리가 첫 인스턴스 — 가드를 프로세스 수명만큼 유지할 것.
/// - `None` = 이미 상주 중 — [`signal_open`]으로 위임하고 종료하라.
///
/// `lock_path`는 Unix 잠금 파일 위치(Windows에선 무시 — 프로필은 `tag`로 가른다).
#[must_use]
pub fn acquire(lock_path: &std::path::Path, tag: Option<&str>) -> Option<InstanceGuard> {
    #[cfg(windows)]
    {
        let _ = lock_path;
        // SAFETY: 실패는 널/0으로 돌아오고 그때마다 빠져나간다.
        unsafe {
            let h = win::CreateMutexW(core::ptr::null(), 0, win::wide(&mutex_name(tag)).as_ptr());
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
        let _ = tag; // Unix는 잠금 파일이 프로필 데이터 폴더 안이라 저절로 갈린다.
        if let Some(dir) = lock_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // 잠금 파일조차 못 열면 가드 없이 진행한다(안 뜨는 것보다 낫다 · DR-31 — Windows 가지와 같은 결).
        //   종전에는 `None`("이미 실행 중")으로 돌려 앱이 뜨지 않았다(10-05).
        let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
        else {
            return Some(InstanceGuard { _file: None });
        };
        // SAFETY: 유효한 fd에 대한 비블로킹 flock — 실패 = 이미 잠김.
        let rc = unsafe { libc_flock(file.as_raw_fd(), 2 | 4) }; // LOCK_EX | LOCK_NB
        if rc != 0 {
            return None;
        }
        Some(InstanceGuard { _file: Some(file) })
    }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "flock"]
    fn libc_flock(fd: i32, op: i32) -> i32;
}

/// 둘째 실행 — 기존 인스턴스에 "열기"를 알린다. 반환 = 알림이 **전달됐는가**(거짓말하지 않는다 · DR-31).
///
/// `sock`은 Unix 소켓 경로(잠금 파일과 같은 폴더 · Windows에선 무시 — 이름 있는 이벤트를 쓴다).
#[must_use]
pub fn signal_open(tag: Option<&str>, sock: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        let _ = sock;
        // SAFETY: 이름으로 열기 실패 = 0 → 알림 실패.
        unsafe {
            let h = win::CreateEventW(
                core::ptr::null(),
                0,
                0,
                win::wide(&open_event_name(tag)).as_ptr(),
            );
            if h == 0 {
                return false;
            }
            let ok = win::SetEvent(h) != 0;
            win::CloseHandle(h);
            ok
        }
    }
    #[cfg(unix)]
    {
        use std::io::Write as _;
        let _ = tag;
        std::os::unix::net::UnixStream::connect(sock)
            .and_then(|mut s| s.write_all(b"open\n"))
            .is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tag, sock);
        false
    }
}

/// 첫 인스턴스 — "열기" 신호를 기다렸다가 `on_open`을 부른다(백그라운드 스레드).
/// `sock` = Unix 소켓 경로([`signal_open`]과 같은 값 · Windows에선 무시).
pub fn watch_open_requests(
    tag: Option<&str>,
    sock: &std::path::Path,
    on_open: impl Fn() + Send + 'static,
) {
    #[cfg(windows)]
    {
        let _ = sock;
        let name = open_event_name(tag);
        std::thread::Builder::new()
            .name("nclip-single".into())
            .spawn(move || {
                // SAFETY: 자동 리셋 이벤트를 만들어(이미 있으면 그 핸들) 무한 대기 루프.
                unsafe {
                    let h = win::CreateEventW(core::ptr::null(), 0, 0, win::wide(&name).as_ptr());
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
    #[cfg(unix)]
    {
        let _ = tag;
        // 잠금을 쥔 쪽만 여기 온다 — 남아 있는 소켓 파일은 죽은 인스턴스의 것이라 지우고 새로 연다.
        let _ = std::fs::remove_file(sock);
        let Ok(listener) = std::os::unix::net::UnixListener::bind(sock) else {
            return; // 경로가 너무 길거나 쓸 수 없는 자리 — 위임 없이 간다(둘째 실행이 정직하게 알린다).
        };
        std::thread::Builder::new()
            .name("nclip-single".into())
            .spawn(move || {
                for conn in listener.incoming() {
                    if conn.is_ok() {
                        on_open();
                    }
                }
            })
            .ok();
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (tag, sock, on_open);
    }
}
