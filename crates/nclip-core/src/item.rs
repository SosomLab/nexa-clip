//! 클립보드 항목 모델 — ★ **다중 표현(representation)** 이 이 프로젝트의 핵심 자료구조다.
//!
//! Word에서 표를 하나 복사하면 클립보드에는 **같은 내용의 여러 표현이 동시에** 올라간다
//! (평문 · HTML · RTF · 이미지 · Office 개체). 붙여넣는 앱이 **자기가 아는 것 중 가장 좋은
//! 것을 골라 간다** — 그래서 우리가 몇 벌을 보관하느냐가 곧 붙여넣기 품질이 된다
//! ([docs/12](../../../docs/12-clipboard-formats.md)).
//!
//! ## 원칙 F-1 — 해석하지 않고 **이름째** 보관한다
//!
//! 알려진 포맷만 골라 담으면 PPT 도형(`Art::GVML ClipFormat`)이 그림으로, Excel 범위가 값으로
//! 떨어진다. **올라온 표현을 이름과 함께 그대로** 들고 있다가 그대로 돌려주면 앱마다 대응할
//! 필요가 없다.

use crate::capture::Preview;

/// 항목 식별자(16B).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct ItemId(pub [u8; 16]);

impl ItemId {
    /// 바이트에서 만든다.
    #[must_use]
    pub const fn from_bytes(b: [u8; 16]) -> Self {
        Self(b)
    }
    /// 원본 바이트.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// 표시·필터용 분류. **저장 형식이 아니라 "사람이 보는 갈래"** 다 —
/// 실제 바이트는 [`Representation`]에 있다.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ClipKind {
    /// 평문.
    Text,
    /// 서식 있는 텍스트(HTML/RTF 또는 Office 개체를 동반).
    RichText,
    /// 이미지.
    Image,
    /// 파일·폴더 경로 목록.
    Files,
    /// 색상 코드(자동 판별).
    Color,
}

impl ClipKind {
    /// i18n 라벨 키.
    #[must_use]
    pub fn label(self) -> crate::Msg {
        match self {
            ClipKind::Text => crate::Msg::KindText,
            ClipKind::RichText => crate::Msg::KindRichText,
            ClipKind::Image => crate::Msg::KindImage,
            ClipKind::Files => crate::Msg::KindFiles,
            ClipKind::Color => crate::Msg::KindColor,
        }
    }
}

/// 표현 하나 — **OS 포맷 이름 + 내용 주소**.
///
/// `blob_id`는 [`docs/06`](../../../docs/06-storage-design.md)의 **내용 주소**(암호문 해시)라
/// 같은 바이트면 저장이 한 번만 일어난다. 인덱스에는 **본문이 없다** — 참조만 있다.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Representation {
    /// OS가 준 포맷 이름 그대로(예: `CF_HTML` · `public.rtf` · `Art::GVML ClipFormat`).
    /// ★ 우리는 이 문자열을 **해석하지 않는다**(F-1).
    pub format: String,
    /// 내용 주소(암호문 해시).
    pub blob_id: [u8; 32],
    /// 바이트 크기(상한 판정·표시용).
    pub bytes: u64,
}

/// 기록 항목 하나.
///
/// ⚠️ **`preview`는 목록 표시 전용**이다. 원문이 아니다 —
/// 전문은 `reps`의 blob을 열어야 한다([docs/06 §2-1](../../../docs/06-storage-design.md)).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipItem {
    /// 식별자.
    pub id: ItemId,
    /// 표시 분류.
    pub kind: ClipKind,
    /// ★ **목록이 읽는 유일한 것**([`Preview`] · [docs/27 §6](../../../docs/27-capture-cases.md)).
    ///
    /// 캡처 때 **한 번** 만든다 — 스크롤마다 3MB DIB를 디코드할 수 없고,
    /// HTML 정제(스크립트 제거)도 여기 한 지점에 모인다.
    pub preview: Preview,
    /// 보유 표현 목록 — **폴백 순서가 아니라 보유 순서**다.
    pub reps: Vec<Representation>,
    /// 생성 시각(UNIX 밀리초).
    pub created_ms: u64,
    /// 마지막으로 복사된 시각 — 중복 재복사 시 **여기만 갱신**하고 새 항목을 만들지 않는다.
    pub last_copied_ms: u64,
    /// 복사 횟수(정렬 축 하나 — [docs/14 §3-4](../../../docs/14-settings-registry.md)).
    pub copy_count: u32,
    /// 고정 여부 — 보관 정책 만료에서 제외된다.
    pub pinned: bool,
    /// 출처 앱 표시 이름(모르면 `None`).
    pub source_app: Option<String>,
}

impl ClipItem {
    /// 평문 표현이 있는가. ★ **F-2 — 폴백은 항상 있어야 한다.**
    #[must_use]
    pub fn has_plain(&self) -> bool {
        self.reps.iter().any(|r| is_plain_format(&r.format))
    }

    /// 평문 표현을 찾는다.
    #[must_use]
    pub fn plain_rep(&self) -> Option<&Representation> {
        self.reps.iter().find(|r| is_plain_format(&r.format))
    }

    /// 전체 바이트(표현 합).
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.reps.iter().map(|r| r.bytes).sum()
    }

    /// ★ 중복 판정 키 — **표현 blob_id의 조합**.
    ///
    /// 정렬해서 모으므로 **표현 순서가 달라도 같은 내용이면 같은 키**가 나온다.
    /// 같은 키를 다시 만나면 새 항목 대신 [`Self::promote`]로 최신 승격한다(FR-C-10).
    #[must_use]
    pub fn dedup_key(&self) -> Vec<[u8; 32]> {
        let mut ids: Vec<[u8; 32]> = self.reps.iter().map(|r| r.blob_id).collect();
        ids.sort_unstable();
        ids
    }

    /// 같은 내용을 다시 복사했을 때 — **새 항목을 만들지 않고 최신으로 올린다**.
    pub fn promote(&mut self, now_ms: u64) {
        self.last_copied_ms = now_ms;
        self.copy_count = self.copy_count.saturating_add(1);
    }

    /// 사다리 꼭대기 표현 — ★ **저장하지 않고 그때그때 구한다**.
    ///
    /// 인덱스를 필드로 들고 있으면 [`select_reps`](crate::capture::select_reps)가
    /// 표현을 버릴 때 **가리키는 자리가 밀린다**. 파생값은 파생으로 둔다.
    #[must_use]
    pub fn primary(&self) -> Option<&Representation> {
        crate::capture::primary_index(&self.reps).map(|i| &self.reps[i])
    }

    /// 목록 한 줄에 쓸 글자 — 종류가 무엇이든 **빈 줄을 주지 않는다**.
    #[must_use]
    pub fn one_line(&self) -> String {
        self.preview.one_line()
    }
}

/// 그 포맷 이름이 **평문**인가(3-OS 이름을 한곳에서 판정).
#[must_use]
pub fn is_plain_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "CF_UNICODETEXT" | "CF_TEXT" | "public.utf8-plain-text" | "text/plain"
    ) || fmt.starts_with("text/plain;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(fmt: &str, seed: u8, bytes: u64) -> Representation {
        Representation {
            format: fmt.to_string(),
            blob_id: [seed; 32],
            bytes,
        }
    }

    fn item(reps: Vec<Representation>) -> ClipItem {
        ClipItem {
            id: ItemId::from_bytes([1; 16]),
            kind: ClipKind::RichText,
            preview: Preview::Text("이름 나이".into()),
            reps,
            created_ms: 1_000,
            last_copied_ms: 1_000,
            copy_count: 1,
            pinned: false,
            source_app: Some("Word".into()),
        }
    }

    #[test]
    fn plain_fallback_is_detected() {
        let it = item(vec![rep("CF_HTML", 2, 400), rep("CF_UNICODETEXT", 3, 40)]);
        assert!(it.has_plain());
        assert_eq!(it.plain_rep().map(|r| r.bytes), Some(40));
    }

    #[test]
    fn missing_plain_is_visible() {
        // ★ 평문이 없으면 F-2 위반 — 캡처 단계에서 파생해 채워야 한다.
        let it = item(vec![rep("Art::GVML ClipFormat", 9, 214_003)]);
        assert!(!it.has_plain());
    }

    #[test]
    fn total_bytes_sums_reps() {
        let it = item(vec![rep("CF_HTML", 2, 400), rep("CF_UNICODETEXT", 3, 40)]);
        assert_eq!(it.total_bytes(), 440);
    }

    /// ★ 표현 순서가 달라도 같은 내용이면 같은 중복 키가 나와야 한다.
    #[test]
    fn dedup_key_is_order_independent() {
        let a = item(vec![rep("CF_HTML", 2, 400), rep("CF_UNICODETEXT", 3, 40)]);
        let b = item(vec![rep("CF_UNICODETEXT", 3, 40), rep("CF_HTML", 2, 400)]);
        assert_eq!(a.dedup_key(), b.dedup_key());
    }

    #[test]
    fn promote_bumps_time_and_count() {
        let mut it = item(vec![rep("CF_UNICODETEXT", 3, 40)]);
        it.promote(5_000);
        assert_eq!(it.last_copied_ms, 5_000);
        assert_eq!(it.copy_count, 2);
        assert_eq!(it.created_ms, 1_000, "생성 시각은 유지된다");
    }
}
