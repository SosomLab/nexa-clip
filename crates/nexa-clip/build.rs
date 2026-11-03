//! ★ 빌드 표식(09-12 · 사용자 요청 "설치본은 두고 실행 파일만 바꿨을 때 어느 빌드인지 알 수 있게") —
//! git 커밋(짧은 SHA · 작업트리 변경 시 `+dirty`)과 빌드 시각을 `env!`로 박는다.
//! 정보 화면(설정 → 정보)이 실행 파일 SHA-256과 함께 보여 준다. git이 없는 소스 아카이브 빌드는 `unknown`.
//! 외부 crate 0 — `git` 명령과 표준 라이브러리만.
//!
//! ★ 11-03: Windows 아이콘·버전·매니페스트 리소스(`packaging/windows/winres.rs` · nclip-imgdec과 공용 · 원본 nexa-sql T-62)도
//!   여기서 링크한다 — VirusTotal 휴리스틱 보조(T-77 · 서명 없는 PE의 버전 정보 공란을 없앤다). rc 도구 없으면 조용히 건너뛴다.

include!("../../packaging/windows/winres.rs");

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    embed_windows_resources("nexa-clip.rc", "nexa-clip");
    let sha = git(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "unknown".into());
    // `git status --porcelain`이 비어 있지 않으면 작업트리에 변경이 있다(개발 빌드 식별).
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some();
    let sha = if dirty && sha != "unknown" {
        format!("{sha}+dirty")
    } else {
        sha
    };
    // 빌드 시각(UTC · 초) — 표준 라이브러리만으로 사람이 읽는 형태까지 만든다.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    println!("cargo:rustc-env=NCLIP_GIT_SHA={sha}");
    println!("cargo:rustc-env=NCLIP_BUILD_UNIX={secs}");
    // HEAD·참조가 바뀌면 다시 돌린다(커밋 뒤 빌드가 옛 SHA를 박지 않게).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rerun-if-changed=../../.git/index");
}
