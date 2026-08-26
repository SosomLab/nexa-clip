//! 진단 로그 — ★ **실패의 기본 통보 채널**([DR-31](../../../docs/10-decision-record.md)).
//!
//! ## 왜 로그인가
//!
//! 모달은 흐름을 끊는다(사용자 확정 — *"모달 창 안내는 지금은 제외"*). 실패한 항목은
//! **상태가 바뀌어 목록에서 보이고**, **왜 그런지와 무엇을 하면 되는지**는 여기 남는다.
//!
//! ## 규칙
//!
//! | # | 규칙 | 왜 |
//! |:--:|---|---|
//! | **L-1** | ★ **원인과 조치를 함께** 남긴다 | *"무엇이 잘못됐나"* 만으로는 **사용자가 할 일을 모른다** |
//! | **L-2** | 로컬 전용 · 외부 전송 0 | [DR-20](../../../docs/10-decision-record.md) |
//! | **L-3** | **상한 있는 링 버퍼** | 24시간 상주 앱이라 무한 증가는 곧 누수다 |
//! | **L-4** | ★ **항목 내용을 남기지 않는다** | 클립보드에는 비밀번호가 지나간다 — id·타입·크기·사유만 |

use std::collections::VecDeque;

/// 심각도.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    /// 참고(정상 흐름의 기록).
    Info,
    /// 주의(동작은 했으나 축소·강등됨).
    Warn,
    /// 실패(요청한 일이 안 됨).
    Error,
}

/// 로그 한 줄.
///
/// ★ **`cause`와 `action`이 쌍이다** — 둘 중 하나만 있으면 이 구조체를 쓸 이유가 없다.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    /// 발생 시각(UNIX 밀리초 — 표시는 호출자가 로컬 시간으로).
    pub at_ms: u64,
    /// 심각도.
    pub level: Level,
    /// 한 줄 요약(무슨 일이 있었나).
    pub what: String,
    /// 대상 식별(★ **내용이 아니라 식별** — "3개 파일 · 출처 A-데스크톱").
    pub subject: Option<String>,
    /// ★ 원인 — 왜 그렇게 됐나.
    pub cause: String,
    /// ★ 조치 — 사용자가 무엇을 하면 되나.
    pub action: String,
}

impl Record {
    /// 사람이 읽는 여러 줄 형태(로그 창·복사 지원).
    #[must_use]
    pub fn render(&self) -> String {
        let mut s = format!("[{:?}] {}", self.level, self.what);
        if let Some(sub) = &self.subject {
            s.push_str(&format!("\n  대상: {sub}"));
        }
        s.push_str(&format!("\n  원인: {}", self.cause));
        s.push_str(&format!("\n  조치: {}", self.action));
        s
    }
}

/// 상한 있는 링 버퍼 로그.
///
/// 가장 오래된 것부터 밀려난다 — **버려진 개수를 세어** 사용자가 *"앞이 잘렸다"* 를 알 수 있게 한다.
#[derive(Debug)]
pub struct DiagLog {
    buf: VecDeque<Record>,
    cap: usize,
    dropped: u64,
}

impl DiagLog {
    /// 상한을 정해 만든다. `cap == 0`이면 1로 올린다(빈 로그는 의미가 없다).
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
            dropped: 0,
        }
    }

    /// 한 줄 남긴다. 상한을 넘으면 **가장 오래된 것이 밀려난다**.
    pub fn push(&mut self, rec: Record) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.buf.push_back(rec);
    }

    /// 최신이 뒤인 순서로 훑는다.
    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        self.buf.iter()
    }

    /// 보관 중인 줄 수.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// 비어 있는가.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// ★ 상한 때문에 **버려진 줄 수** — 0이 아니면 로그 창 상단에 알린다.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// 전부 지운다(버려진 개수도 초기화).
    pub fn clear(&mut self) {
        self.buf.clear();
        self.dropped = 0;
    }

    /// 심각도 하한으로 걸러 본다(로그 창 필터).
    pub fn filtered(&self, min: Level) -> impl Iterator<Item = &Record> {
        self.buf.iter().filter(move |r| r.level >= min)
    }
}

impl Default for DiagLog {
    /// 기본 상한 **500줄** — 상주 앱에서 메모리를 눈에 띄게 먹지 않는 선.
    fn default() -> Self {
        Self::with_capacity(500)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(level: Level, what: &str) -> Record {
        Record {
            at_ms: 1,
            level,
            what: what.into(),
            subject: Some("3개 파일 · 출처 A-데스크톱".into()),
            cause: "원본 기기와 세션이 없습니다".into(),
            action: "A-데스크톱을 켜거나 사전 캐시 크기를 늘리세요".into(),
        }
    }

    /// ★ 상한을 넘으면 오래된 것이 밀려나고 **버려진 수가 세어진다**.
    #[test]
    fn ring_buffer_drops_oldest_and_counts() {
        let mut log = DiagLog::with_capacity(2);
        log.push(rec(Level::Info, "1"));
        log.push(rec(Level::Info, "2"));
        log.push(rec(Level::Info, "3"));
        assert_eq!(log.len(), 2);
        assert_eq!(log.dropped(), 1, "버려진 줄을 세어야 앞이 잘린 걸 알린다");
        let whats: Vec<_> = log.iter().map(|r| r.what.as_str()).collect();
        assert_eq!(whats, ["2", "3"], "최신이 뒤에 남는다");
    }

    /// 상한 0은 1로 올린다 — 빈 로그는 의미가 없다.
    #[test]
    fn zero_capacity_becomes_one() {
        let mut log = DiagLog::with_capacity(0);
        log.push(rec(Level::Warn, "x"));
        assert_eq!(log.len(), 1);
    }

    /// 심각도 필터가 하한 이상만 준다.
    #[test]
    fn filter_by_level() {
        let mut log = DiagLog::default();
        log.push(rec(Level::Info, "i"));
        log.push(rec(Level::Warn, "w"));
        log.push(rec(Level::Error, "e"));
        let warn_up: Vec<_> = log.filtered(Level::Warn).map(|r| r.what.as_str()).collect();
        assert_eq!(warn_up, ["w", "e"]);
    }

    /// ★ 렌더에 **원인과 조치가 둘 다** 나온다 — 하나만 있으면 사용자가 막힌다.
    #[test]
    fn render_has_cause_and_action() {
        let out = rec(Level::Error, "파일 받기 불가").render();
        assert!(out.contains("원인:"), "원인이 없다");
        assert!(
            out.contains("조치:"),
            "★ 조치가 없으면 사용자가 할 일을 모른다"
        );
        assert!(out.contains("파일 받기 불가"));
    }

    /// 비우면 버려진 개수도 함께 초기화된다.
    #[test]
    fn clear_resets_dropped() {
        let mut log = DiagLog::with_capacity(1);
        log.push(rec(Level::Info, "a"));
        log.push(rec(Level::Info, "b"));
        assert_eq!(log.dropped(), 1);
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.dropped(), 0);
    }
}
