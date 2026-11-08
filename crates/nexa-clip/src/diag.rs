//! ★ 앱 전역 진단 기록(S9 · T-22b · 11-08) — [`nclip_core::diag::DiagLog`] 한 벌을 프로세스가 공유한다.
//!
//! [DR-31](../../../docs/10-decision-record.md): 실패는 모달이 아니라 **상태 전이 + 로그**로 알린다 — 여기 남은 기록을
//! 설정 ▸ 고급 ▸ \[로그 보기…\](`nclip_ui::log_win`)가 보여 준다. 콘솔 `eprintln!`(= `diag.log` · T-42)은 그대로 두고,
//! **사용자가 할 일이 있는 실패**만 여기 적는다(L-1 원인+조치 · L-4 항목 내용 금지 — id·종류·크기·사유만).

use nclip_core::diag::{DiagLog, Level, Record};
use nclip_core::{t, Msg};
use std::sync::Mutex;

static LOG: Mutex<Option<DiagLog>> = Mutex::new(None);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// 한 줄 남긴다 — `what`·`action`은 i18n(지금 언어로 풀어 저장) · `cause`는 오류 원문(상태만 · 내용 없음).
pub(crate) fn record(level: Level, what: Msg, subject: Option<String>, cause: String, action: Msg) {
    let rec = Record {
        at_ms: now_ms(),
        level,
        what: t(what).to_string(),
        subject,
        cause,
        action: t(action).to_string(),
    };
    if let Ok(mut g) = LOG.lock() {
        g.get_or_insert_with(DiagLog::default).push(rec);
    }
}

/// 스냅숏(오래된 것부터) + 상한 때문에 버려진 수.
pub(crate) fn snapshot() -> (Vec<Record>, u64) {
    LOG.lock().map_or((Vec::new(), 0), |g| {
        g.as_ref().map_or((Vec::new(), 0), |l| {
            (l.iter().cloned().collect(), l.dropped())
        })
    })
}

/// 전부 지운다(로그 화면 \[지우기\]).
pub(crate) fn clear() {
    if let Ok(mut g) = LOG.lock() {
        if let Some(l) = g.as_mut() {
            l.clear();
        }
    }
}

/// 보관 중인 줄 수(시험·덤프).
#[cfg(test)]
pub(crate) fn len() -> usize {
    LOG.lock().map_or(0, |g| g.as_ref().map_or(0, DiagLog::len))
}

/// UNIX 밀리초 → 로컬 `HH:MM:SS`(라이브러리 없이 — 시스템 UTC 오프셋은 [`nclip_plat::host::utc_offset_secs`]).
pub(crate) fn hms_local(at_ms: u64) -> String {
    let secs =
        i64::try_from(at_ms / 1000).unwrap_or(0) + i64::from(nclip_plat::host::utc_offset_secs());
    let day = secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_clear() {
        clear();
        record(
            Level::Warn,
            Msg::DiagWhatSync,
            None,
            "x".into(),
            Msg::DiagActSyncRetry,
        );
        assert!(len() >= 1);
        let (v, dropped) = snapshot();
        assert_eq!(dropped, 0);
        assert!(v.iter().any(|r| r.cause == "x"));
        clear();
        assert_eq!(len(), 0);
    }

    #[test]
    fn hms_wraps_day() {
        let s = hms_local(0);
        assert_eq!(s.len(), 8);
        assert_eq!(s.as_bytes()[2], b':');
    }
}
