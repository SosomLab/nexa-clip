//! **사용자 폴더 위치** — 실체화 기본 대상(다운로드 폴더 · [docs/11] §4 실체화 절차 1).
//!
//! 외부 크레이트 없이 환경 변수만 본다(런타임 의존 0 · DR-5). 표준 위치가 없거나
//! 만들 수 없으면 **호출자가 판단할 수 있게** `None`을 준다 — 임의 폴더에 몰래 쓰지 않는다.

use std::path::PathBuf;

/// 홈 디렉터리(`$HOME` · 윈도우 `%USERPROFILE%`).
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

/// 다운로드 폴더 — 실체화 기본 대상. 없으면 `None`(호출자가 대안을 정한다).
///
/// Windows `%USERPROFILE%\Downloads` · macOS `~/Downloads` · ★ Linux는 XDG 사용자 폴더
/// (`$XDG_DOWNLOAD_DIR` → `~/.config/user-dirs.dirs`의 `XDG_DOWNLOAD_DIR`)를 먼저 보고
/// 없으면 `~/Downloads`(09-13 — 받은 파일 저장 폴더 기본값이 OS 다운로드 폴더가 되면서).
#[must_use]
pub fn downloads_dir() -> Option<PathBuf> {
    let home = home_dir()?;
    #[cfg(target_os = "linux")]
    if let Some(d) = xdg_download_dir(&home) {
        if d.is_dir() {
            return Some(d);
        }
    }
    let d = home.join("Downloads");
    d.is_dir().then_some(d)
}

/// Linux XDG 다운로드 폴더 — 환경 변수 → `user-dirs.dirs` 순. 절대 경로만 받는다.
#[cfg(target_os = "linux")]
fn xdg_download_dir(home: &std::path::Path) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return Some(p);
        }
    }
    let cfg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let text = std::fs::read_to_string(cfg.join("user-dirs.dirs")).ok()?;
    parse_user_dirs_download(&text, home)
}

/// `user-dirs.dirs`에서 `XDG_DOWNLOAD_DIR="$HOME/…"` 한 줄을 읽는다(따옴표·`$HOME` 전개).
/// OS 무관 순수 함수 — 3-OS 모두에서 테스트된다.
#[must_use]
pub fn parse_user_dirs_download(text: &str, home: &std::path::Path) -> Option<PathBuf> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("XDG_DOWNLOAD_DIR="))?;
    let raw = line["XDG_DOWNLOAD_DIR=".len()..].trim().trim_matches('"');
    let p = match raw.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(raw),
    };
    p.is_absolute().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_a_real_directory_when_present() {
        // 환경에 따라 없을 수 있다 — 있으면 반드시 실제 디렉터리여야 한다.
        if let Some(h) = home_dir() {
            assert!(h.is_dir(), "홈이 디렉터리가 아니다: {}", h.display());
        }
    }

    /// ★ XDG `user-dirs.dirs` 파서(09-13) — `$HOME` 전개 · 따옴표 · 주석/다른 키 무시 · 상대 경로 거부.
    #[test]
    fn user_dirs_download_line_is_parsed() {
        let home = std::path::Path::new("/home/u");
        let text =
            "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/받기\"\n";
        assert_eq!(
            parse_user_dirs_download(text, home),
            Some(PathBuf::from("/home/u/받기"))
        );
        assert_eq!(
            parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"/mnt/dl\"", home),
            Some(PathBuf::from("/mnt/dl"))
        );
        assert_eq!(
            parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"rel\"", home),
            None
        );
        assert_eq!(
            parse_user_dirs_download("XDG_DESKTOP_DIR=\"$HOME/D\"", home),
            None
        );
    }

    #[test]
    fn downloads_is_under_home_when_present() {
        if let (Some(h), Some(d)) = (home_dir(), downloads_dir()) {
            assert!(d.starts_with(&h), "다운로드가 홈 밖: {}", d.display());
            assert!(d.is_dir());
        }
    }
}
