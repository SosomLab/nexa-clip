//! ★ **정보(About)**(09-12 · 사용자 요청) — 설정 → 정보 화면의 보고 행. 버전·빌드(git SHA·시각)·
//! 실행 파일 경로·크기·**SHA-256**·OS·데이터 폴더.
//!
//! 목적: *"설치본은 두고 실행 파일만 바꿨을 때 어느 빌드인지 알 수 있게"* — 버전 문자열은 같아도
//! (`0.1.3`) 실행 파일 해시와 git SHA는 다르다. 실기 보고·릴리스 대조에 그대로 붙여 넣는다.
//!
//! 해시는 파일 전체(≈1.7MB)를 읽어야 하므로 **부팅 때 워커 스레드가 한 번** 계산해 둔다(DR-41 —
//! 설정 창을 여는 UI 스레드는 결과만 읽는다). 계산 전엔 "계산 중…"이 보이고 다음 폴에서 채워진다.

use nclip_core::{tr, Lang, Msg};
use std::sync::OnceLock;

/// 실행 파일 요약 — 경로 · 크기 · 수정시각(unix 초) · SHA-256(16진 64자).
struct ExeInfo {
    path: String,
    size: u64,
    mtime: u64,
    sha256: String,
}

static EXE: OnceLock<ExeInfo> = OnceLock::new();

/// 부팅 때 한 번 — 실행 파일을 읽어 해시한다(워커 스레드 · 실패는 "unknown").
pub(crate) fn spawn_hash() {
    let _ = std::thread::Builder::new()
        .name("nclip-about-hash".into())
        .spawn(|| {
            use sha2::{Digest, Sha256};
            let Ok(p) = std::env::current_exe() else {
                return;
            };
            let meta = std::fs::metadata(&p).ok();
            let sha256 = std::fs::read(&p).map_or_else(
                |_| "unknown".to_string(),
                |b| format!("{:x}", Sha256::digest(&b)),
            );
            let _ = EXE.set(ExeInfo {
                path: p.to_string_lossy().into_owned(),
                size: meta.as_ref().map_or(0, std::fs::Metadata::len),
                mtime: meta.as_ref().map_or(0, crate::xfer::mtime_secs),
                sha256,
            });
        });
}

/// unix 초 → `YYYY-MM-DD HH:MM` (UTC · 외부 crate 없이 — 날짜 산술은 civil-from-days 공식).
fn fmt_utc(secs: u64) -> String {
    if secs == 0 {
        return "-".into();
    }
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m) = (rem / 3600, (rem % 3600) / 60);
    // Howard Hinnant의 civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02} UTC")
}

/// 천 단위 구분 바이트.
fn fmt_bytes(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 보고 행 본문(개행 구분 · 줄 앞 `*` = 강조).
pub(crate) fn report(lang: Lang) -> String {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut lines = vec![
        format!(
            "*{} {} ({profile})",
            tr(lang, Msg::AppName),
            env!("CARGO_PKG_VERSION")
        ),
        format!(
            "{}: {} · {}",
            tr(lang, Msg::AboutBuild),
            env!("NCLIP_GIT_SHA"),
            fmt_utc(env!("NCLIP_BUILD_UNIX").parse().unwrap_or(0))
        ),
    ];
    match EXE.get() {
        Some(e) => {
            lines.push(format!("{}: {}", tr(lang, Msg::AboutExe), e.path));
            lines.push(format!(
                "{}: {} B · {}",
                tr(lang, Msg::AboutSize),
                fmt_bytes(e.size),
                fmt_utc(e.mtime)
            ));
            // 64자 해시는 두 줄로(설정 창 폭에서 한 줄에 안 들어간다) — 앞 8자를 강조해 눈으로 대조.
            let (a, b) = e.sha256.split_at(e.sha256.len().min(32));
            lines.push(format!("*{}: {a}", tr(lang, Msg::AboutHash)));
            if !b.is_empty() {
                lines.push(format!("  {b}"));
            }
        }
        None => lines.push(format!(
            "{}: {}",
            tr(lang, Msg::AboutHash),
            tr(lang, Msg::AboutHashPending)
        )),
    }
    lines.push(format!(
        "{}: {} {}",
        tr(lang, Msg::AboutOs),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    lines.push(format!(
        "{}: {}",
        tr(lang, Msg::AboutData),
        crate::conf::data_dir().to_string_lossy()
    ));
    lines.join("\n")
}

/// 복사용 평문 — 강조 표식을 뗀다.
pub(crate) fn report_plain(lang: Lang) -> String {
    report(lang)
        .lines()
        .map(|l| l.strip_prefix('*').unwrap_or(l).trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formatting_is_civil() {
        assert_eq!(fmt_utc(0), "-");
        assert_eq!(fmt_utc(1_757_700_000), "2025-09-12 18:00 UTC");
        assert_eq!(fmt_utc(951_782_400), "2000-02-29 00:00 UTC"); // 윤일
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(fmt_bytes(0), "0");
        assert_eq!(fmt_bytes(999), "999");
        assert_eq!(fmt_bytes(1_694_208), "1,694,208");
    }

    /// 빌드 표식이 박혀 있고, 보고 행은 강조 줄로 시작하며 복사본은 표식이 없다.
    #[test]
    fn report_has_version_build_and_plain_copy() {
        assert!(!env!("NCLIP_GIT_SHA").is_empty());
        let r = report(Lang::Ko);
        assert!(r.starts_with("*Nexa Clip "), "{r}");
        assert!(r.contains(env!("NCLIP_GIT_SHA")));
        assert!(!report_plain(Lang::Ko).contains('*'));
    }
}
