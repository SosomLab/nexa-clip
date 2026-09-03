//! 표시 이름 — **무해화된** 기기 이름(09-03 사용자 요청 "Display name으로 같은 UserId의
//! 기기를 구별"). beep `nbeep-core/src/name.rs` 이식 사본(도메인만 clip).
//!
//! 이름은 신원이 아니다(신원은 [`crate::PeerId`]). 화면·목록에 뜨는 문자열일 뿐이며,
//! **양방향 오버라이드(RLO 등)·제어문자·0폭 문자를 제거**해 표시가 실제와 달라 보이는 위장을 막는다.
//! 생성에 성공했다는 사실이 곧 무해화 완료다 — 이후 도메인은 이름을 다시 검사하지 않는다.

use core::fmt;

/// 표시 이름의 문자 수 상한(무해화 후 기준).
pub const MAX_NAME_CHARS: usize = 64;

/// 이름 무해화 실패 사유.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// 무해화 후 빈 문자열(표시할 게 없음).
    Empty,
    /// 상한 초과.
    TooLong,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameError::Empty => f.write_str("이름이 비어 있음(무해화 후)"),
            NameError::TooLong => f.write_str("이름이 너무 김"),
        }
    }
}

impl std::error::Error for NameError {}

/// 무해화된 표시 이름. `parse`로만 만들 수 있어 **항상 안전**하다.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DisplayName(String);

impl DisplayName {
    /// 원시 문자열을 무해화해 표시 이름을 만든다.
    ///
    /// 제거: **양방향 제어**(U+202A–202E · U+2066–2069) · **0폭**(U+200B–200D · U+FEFF) ·
    /// 기타 제어문자(개행·탭 포함). 앞뒤 공백은 트림하고 연속 공백은 한 칸으로 접는다.
    ///
    /// # Errors
    /// 무해화 후 비면 [`NameError::Empty`], 상한 초과면 [`NameError::TooLong`].
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        let mut out = String::with_capacity(raw.len());
        let mut prev_space = false;
        for ch in raw.chars() {
            if is_stripped(ch) {
                continue;
            }
            if ch.is_whitespace() {
                if !out.is_empty() && !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
                continue;
            }
            out.push(ch);
            prev_space = false;
        }
        if out.ends_with(' ') {
            out.pop();
        }
        if out.is_empty() {
            return Err(NameError::Empty);
        }
        if out.chars().count() > MAX_NAME_CHARS {
            return Err(NameError::TooLong);
        }
        Ok(Self(out))
    }

    /// 무해화된 문자열.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayName({:?})", self.0)
    }
}

/// 표시에서 통째로 제거하는 문자 — 양방향 제어·0폭·비공백 제어.
fn is_stripped(ch: char) -> bool {
    matches!(ch,
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        | '\u{200B}'..='\u{200D}' | '\u{FEFF}'
    ) || (ch.is_control() && !ch.is_whitespace())
}

/// 정제 기준 장치 단어 — 이 단어가 나오면 **그 앞은 사람 이름 추정 부분**으로 보고 버린다.
const DEVICE_WORDS: &[&str] = &[
    "macbook",
    "mac",
    "imac",
    "macmini",
    "macstudio",
    "pc",
    "desktop",
    "laptop",
    "notebook",
    "workstation",
    "surface",
    "thinkpad",
    "server",
    "tower",
    "book",
    "맥북",
    "맥",
    "아이맥",
    "노트북",
    "데스크탑",
    "데스크톱",
    "컴퓨터",
    "피씨",
];

/// 호스트명에서 실명 추정 부분을 정제한다(beep FR-S-50 승계).
///
/// 첫 DNS 라벨만 취해 `-`/`_`/공백으로 토큰화 → **처음 나오는 장치 단어부터 끝까지**를
/// `-`로 이어 돌려준다("Sangyongs-MacBook-Pro" → "MacBook-Pro" · "DESKTOP-AB12CD"는
/// 장치 단어(desktop)부터라 그대로). 장치 단어가 없으면 `None`(호스트명 전체가 실명일 수
/// 있어 판별 불가는 버린다 — fail-closed).
#[must_use]
pub fn neutral_from_host(raw: &str) -> Option<String> {
    let label = raw.split('.').next().unwrap_or("");
    let tokens: Vec<&str> = label
        .split(['-', '_', ' '])
        .filter(|t| !t.is_empty())
        .collect();
    let start = tokens.iter().position(|t| {
        let lower = t.to_ascii_lowercase();
        DEVICE_WORDS.iter().any(|w| lower == *w || *t == *w)
    })?;
    let joined = tokens[start..].join("-");
    (!joined.is_empty()).then_some(joined)
}

/// 기본 표시 이름 — 정제된 호스트명, 실패 시 **지문 기반 중립 라벨**(`clip-{지문4}`)로 폴백.
/// 어느 쪽도 실명을 싣지 않는다. 호스트명 취득은 플랫폼 경계(nclip-plat) 몫이라 인자로 받는다.
#[must_use]
pub fn default_display_name(raw_host: Option<&str>, peer: &crate::PeerId) -> DisplayName {
    if let Some(host) = raw_host {
        if let Some(neutral) = neutral_from_host(host) {
            if let Ok(name) = DisplayName::parse(&neutral) {
                return name;
            }
        }
    }
    let hex = crate::relay::peer_hex(peer);
    DisplayName::parse(&format!("clip-{}", &hex[..4]))
        .unwrap_or_else(|_| DisplayName::parse("clip").expect("고정 문자열"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_name_passes() {
        assert_eq!(
            DisplayName::parse("홍길동-맥북").unwrap().as_str(),
            "홍길동-맥북"
        );
    }

    #[test]
    fn neutral_strips_personal_prefix() {
        assert_eq!(
            neutral_from_host("Sangyongs-MacBook-Pro.local").as_deref(),
            Some("MacBook-Pro")
        );
        assert_eq!(
            neutral_from_host("DESKTOP-AB12CD").as_deref(),
            Some("DESKTOP-AB12CD")
        );
        assert_eq!(
            neutral_from_host("gildong-hong"),
            None,
            "장치 단어 없음 = 판별 불가"
        );
    }

    #[test]
    fn bidi_and_zero_width_are_stripped() {
        let n = DisplayName::parse("a\u{202E}b\u{200B}  c\n").unwrap();
        assert_eq!(n.as_str(), "ab c");
        assert_eq!(DisplayName::parse("\u{200B} \t"), Err(NameError::Empty));
    }

    #[test]
    fn fallback_is_fingerprint_label() {
        let id = crate::Identity::generate();
        let n = default_display_name(Some("gildong-hong"), &id.peer_id());
        assert!(n.as_str().starts_with("clip-"), "{n}");
        assert_eq!(n.as_str().len(), 9);
    }
}
