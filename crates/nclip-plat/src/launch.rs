//! OS 기본 프로그램으로 파일 열기(10-10 · 설정 창 하단 \[설정 파일 열기…\] — nexa-beep `nbeep-plat::launch` 선례).
//!
//! 셸 연동만 한다 — Windows = `explorer`(연결 프로그램으로 연다 · 콘솔 창 없음) · macOS = `open` · Linux = `xdg-open`.
//! 실패(도구 없음·스폰 실패)는 `false` — 호출자가 경로를 상태줄에 보여 준다(사용자가 직접 연다).

use std::path::Path;
use std::process::Command;

/// `path`를 OS 기본 프로그램으로 연다. 스폰에 성공하면 `true`(열렸는지까지는 모른다).
#[must_use]
pub fn open_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        Command::new("explorer").arg(path).spawn().is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().is_ok()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().is_ok()
    }
}
