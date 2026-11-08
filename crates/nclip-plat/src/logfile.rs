//! ★ 진단 로그 파일 싱크(T-42 · 11-08) — 프로세스 **stdout·stderr를 파일로 돌린다**.
//!
//! ## 왜 표준 핸들째인가
//!
//! 설치본에는 콘솔이 없다(Windows = `windows` 서브시스템 · mac = Finder/launchd 기동 → `/dev/null`).
//! 앱의 진단 출력은 `println!`·`eprintln!` 백여 곳에 흩어져 있어, 하나하나 싱크 호출로 바꾸면
//! 빠지는 곳이 생기고 패닉 메시지(std가 stderr에 직접 씀)는 잡을 수도 없다. 표준 핸들을
//! 통째로 파일에 잇으면 **기존 출력 무수정 + 패닉까지** 한 번에 남는다. 외부 crate 0(DR-8).
//!
//! - Unix: `dup`으로 원래 fd 1·2를 떠 두고 `dup2(file, 1|2)` · 끄면 되돌린다.
//! - Windows: `GetStdHandle`로 떠 두고 `SetStdHandle` — Rust std는 **쓸 때마다** `GetStdHandle`을
//!   다시 부르므로 바꾼 핸들이 다음 `println!`부터 먹는다.
//!
//! ## 규칙
//!
//! - **로컬 전용**(L-2) — 데이터 폴더의 파일 하나 · 어디로도 보내지 않는다.
//! - **크기 상한**(L-3) — 켤 때와 [`maintain`] 때 상한을 넘었으면 `<이름>.1`로 한 벌만 돌린다
//!   (최대 ≈ 2 × 상한).
//! - **내용 금지**(L-4)는 출력하는 쪽 책임 — 이미 `log_item`이 항목을 `<글>`처럼 종류만 찍는다.
//! - 켜져 있는 동안 터미널(부모 콘솔) 출력은 파일로 간다 — 끄면 원래 대상으로 돌아온다.

use std::fs::File;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 기본 상한 — 2 MB(회전본 포함 최대 ≈ 4 MB).
pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;

struct State {
    path: PathBuf,
    /// 표준 핸들이 가리키는 파일 — Windows는 핸들을 빌려 주므로 **살아 있어야** 한다.
    _file: File,
    saved: imp::Saved,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 지금 파일로 돌리고 있는가.
#[must_use]
pub fn is_on() -> bool {
    lock().is_some()
}

/// 켠다 — 이미 같은 경로로 켜져 있으면 아무것도 안 한다(`Ok(false)`). 새로 켰으면 `Ok(true)`.
///
/// `banner`는 파일에 먼저 한 줄로 남긴다(버전·OS 등 — 호출자가 만든다).
///
/// # Errors
/// 파일을 못 열거나 표준 핸들을 못 바꾸면 — 이때 표준 핸들은 원래대로다.
pub fn enable(path: &Path, max_bytes: u64, banner: &str) -> io::Result<bool> {
    let mut st = lock();
    if st.as_ref().is_some_and(|s| s.path == path) {
        return Ok(false);
    }
    let file = open_rotated(path, max_bytes, banner)?;
    flush_std();
    match st.take() {
        // 다른 경로로 켜져 있었다 — 원래 핸들은 그대로 두고 대상만 옮긴다.
        Some(old) => {
            imp::point(&file)?;
            *st = Some(State {
                path: path.to_path_buf(),
                _file: file,
                saved: old.saved,
            });
        }
        None => {
            let saved = imp::redirect(&file)?;
            *st = Some(State {
                path: path.to_path_buf(),
                _file: file,
                saved,
            });
        }
    }
    Ok(true)
}

/// 끈다 — 표준 핸들을 켜기 전 대상으로 돌린다. 꺼져 있었으면 `false`.
pub fn disable() -> bool {
    let mut st = lock();
    let Some(s) = st.take() else {
        return false;
    };
    flush_std();
    imp::restore(s.saved);
    true
}

/// 주기 점검 — 켜져 있고 파일이 상한을 넘었으면 `<이름>.1`로 돌리고 새 파일로 옮긴다.
/// 돌렸으면 `Ok(true)`.
///
/// # Errors
/// 새 파일을 못 열거나 핸들을 못 옮기면 — 이때는 옛 파일에 계속 쓴다.
pub fn maintain(max_bytes: u64, banner: &str) -> io::Result<bool> {
    let mut st = lock();
    let Some(s) = st.as_mut() else {
        return Ok(false);
    };
    let len = std::fs::metadata(&s.path).map_or(0, |m| m.len());
    if len <= max_bytes {
        return Ok(false);
    }
    flush_std();
    let file = open_rotated(&s.path, max_bytes, banner)?;
    imp::point(&file)?;
    s._file = file; // 옛 파일은 여기서 닫힌다(표준 핸들은 이미 새 파일).
    Ok(true)
}

/// 회전본 경로 — `diag.log` → `diag.log.1`.
#[must_use]
pub fn rotated_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".1");
    PathBuf::from(s)
}

/// 상한을 넘었으면 한 벌 돌리고, 이어 쓰기로 열어 머리 줄을 남긴다.
fn open_rotated(path: &Path, max_bytes: u64, banner: &str) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > max_bytes) {
        // 실패해도 이어 쓴다 — 로그를 못 남기는 것보다 크기를 넘는 편이 낫다.
        let _ = std::fs::rename(path, rotated_path(path));
    }
    let mut f = File::options().create(true).append(true).open(path)?;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    writeln!(
        f,
        "──── {banner} · unix {secs} · pid {} ────",
        std::process::id()
    )?;
    Ok(f)
}

/// 대상을 바꾸기 전에 std 버퍼(stdout = 줄 버퍼)를 비운다 — 안 그러면 반쯤 찬 줄이 엉뚱한 곳으로 간다.
fn flush_std() {
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
}

#[cfg(unix)]
mod imp {
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd as _;
    use std::os::raw::c_int;

    extern "C" {
        fn dup(fd: c_int) -> c_int;
        fn dup2(src: c_int, dst: c_int) -> c_int;
        fn close(fd: c_int) -> c_int;
    }

    /// 켜기 전 fd 1·2의 사본(-1 = 원래 닫혀 있었음).
    pub(super) struct Saved {
        out: c_int,
        err: c_int,
    }

    pub(super) fn redirect(f: &File) -> io::Result<Saved> {
        // SAFETY: 표준 fd 복제 — 실패하면 -1(그때는 되돌릴 때 /dev/null로).
        let saved = unsafe {
            Saved {
                out: dup(1),
                err: dup(2),
            }
        };
        if let Err(e) = point(f) {
            restore(saved);
            return Err(e);
        }
        Ok(saved)
    }

    pub(super) fn point(f: &File) -> io::Result<()> {
        let fd = f.as_raw_fd();
        for dst in [1, 2] {
            // SAFETY: `fd`는 살아 있는 파일 · dst는 표준 fd — dup2는 원자적으로 교체한다.
            if unsafe { dup2(fd, dst) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub(super) fn restore(s: Saved) {
        for (saved, dst) in [(s.out, 1), (s.err, 2)] {
            if saved >= 0 {
                // SAFETY: 우리가 dup으로 만든 fd를 표준 fd로 되돌리고 사본은 닫는다.
                unsafe {
                    dup2(saved, dst);
                    close(saved);
                }
            } else if let Ok(null) = File::options().write(true).open("/dev/null") {
                // 원래 닫혀 있었다 — 파일을 계속 붙잡지 않도록 /dev/null로.
                // SAFETY: 살아 있는 fd → 표준 fd 교체.
                unsafe {
                    dup2(null.as_raw_fd(), dst);
                }
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle as _;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetStdHandle(which: u32, handle: isize) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    /// 켜기 전 표준 핸들(0·-1 = 없음 — 되돌리면 std가 조용히 버린다).
    pub(super) struct Saved {
        out: isize,
        err: isize,
    }

    pub(super) fn redirect(f: &File) -> io::Result<Saved> {
        // SAFETY: 표준 핸들 조회 — 소유권·부작용 없음.
        let saved = unsafe {
            Saved {
                out: GetStdHandle(STD_OUTPUT_HANDLE),
                err: GetStdHandle(STD_ERROR_HANDLE),
            }
        };
        if let Err(e) = point(f) {
            restore(saved);
            return Err(e);
        }
        Ok(saved)
    }

    pub(super) fn point(f: &File) -> io::Result<()> {
        let h = f.as_raw_handle() as isize;
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: 살아 있는 파일 핸들을 빌려 준다 — 파일은 State가 붙잡고 있다.
            if unsafe { SetStdHandle(which, h) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub(super) fn restore(s: Saved) {
        // SAFETY: 켜기 전 값 그대로 — 콘솔 핸들이거나 0(없음).
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, s.out);
            SetStdHandle(STD_ERROR_HANDLE, s.err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ⚠ enable/disable은 프로세스 전역 표준 핸들을 바꾼다 — 병렬 시험 하네스 출력을 흐트러뜨리므로
    //   단위 시험은 파일 쪽(회전·머리 줄)만 본다. 핸들 교체는 Debug 실기로(docs/21 §15).

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nclip-logfile-{}-{name}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&d).unwrap();
        d.join("diag.log")
    }

    #[test]
    fn rotated_path_appends_suffix() {
        assert_eq!(
            rotated_path(Path::new("/a/diag.log")),
            PathBuf::from("/a/diag.log.1")
        );
    }

    #[test]
    fn open_appends_banner() {
        let p = tmp("banner");
        drop(open_rotated(&p, 1024, "x 1").unwrap());
        drop(open_rotated(&p, 1024, "x 2").unwrap());
        let s = std::fs::read_to_string(&p).unwrap();
        assert_eq!(s.lines().count(), 2);
        assert!(s.lines().next().unwrap().contains("x 1"));
        assert!(!rotated_path(&p).exists());
    }

    #[test]
    fn over_limit_rotates_once() {
        let p = tmp("rotate");
        std::fs::write(&p, vec![b'a'; 100]).unwrap();
        std::fs::write(rotated_path(&p), b"old").unwrap();
        drop(open_rotated(&p, 50, "new").unwrap());
        // 넘친 본문이 .1로(옛 .1은 덮임) · 새 파일은 머리 줄만.
        assert_eq!(std::fs::metadata(rotated_path(&p)).unwrap().len(), 100);
        let s = std::fs::read_to_string(&p).unwrap();
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("new"));
    }
}
