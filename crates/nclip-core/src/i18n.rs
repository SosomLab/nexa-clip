//! i18n — **프로세스 전역 로케일 + 메시지 카탈로그**(영어 기본 · 한/일 언어팩).
//!
//! ★ **형태를 `nexa-beep` `nbeep-core::i18n`과 동형으로 맞춘다**([docs/13 §5](../../../docs/13-ui-reuse-from-beep.md)) —
//! 이식해 오는 `settings.rs`·컨트롤이 `tr(lang, Msg::…)` 계약에 의존하므로, 같은 모양이면
//! **`use` 경로만 바꾸면 된다**(D-35).
//!
//! 외부 i18n 크레이트를 쓰지 않는다(DR-8). 문자열은 전부 `&'static str`이라 힙 할당·파일
//! 로드가 없다 — 예산 게이트(DR-9)에 무해하다.
//!
//! ## 카탈로그 형태 — "한 키 = 한 줄 3언어"
//!
//! [`Msg`]의 각 변형이 `[en, ko, ja]`를 한 줄로 갖는다([`Msg::row`]). 한 줄에 모여 있어
//! **누락·불일치를 리뷰에서 바로 본다**. 새 UI 문자열 = 변형 1개 + 줄 1개.

use core::sync::atomic::{AtomicU8, Ordering};

/// 지원 언어 — **영어 기본**.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Lang {
    /// 영어(기본·폴백).
    #[default]
    En,
    /// 한국어.
    Ko,
    /// 일본어.
    Ja,
}

impl Lang {
    /// 전 언어(설정 콤보·순회용).
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Ko, Lang::Ja];

    /// 값 코드(설정 저장·복원 계약).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
            Lang::Ja => "ja",
        }
    }

    /// 코드 → 언어(미지 코드는 `None` — 호출자가 기본으로 폴백).
    #[must_use]
    pub fn from_code(s: &str) -> Option<Lang> {
        match s {
            "en" => Some(Lang::En),
            "ko" => Some(Lang::Ko),
            "ja" => Some(Lang::Ja),
            _ => None,
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

/// 현재 언어를 바꾼다(프로세스 전역).
pub fn set_lang(lang: Lang) {
    let v = match lang {
        Lang::En => 0,
        Lang::Ko => 1,
        Lang::Ja => 2,
    };
    LANG.store(v, Ordering::Relaxed);
}

/// 현재 언어.
#[must_use]
pub fn current_lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Ko,
        2 => Lang::Ja,
        _ => Lang::En,
    }
}

/// UI 문자열 키. **화면에 나가는 문자열은 전부 여기를 거친다** — 하드코딩 금지.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum Msg {
    /// 앱 이름.
    AppName,
    /// 검색 입력 안내문.
    SearchPlaceholder,
    /// 항목이 하나도 없을 때.
    EmptyHistory,
    /// 검색 결과 없음.
    NoMatches,

    // ── 타입 라벨 ─────────────────────────────
    /// 텍스트 항목.
    KindText,
    /// 서식 있는 텍스트.
    KindRichText,
    /// 이미지 항목.
    KindImage,
    /// 파일·폴더 목록 항목.
    KindFiles,
    /// 색상 항목.
    KindColor,

    // ── 보기 모드(FR-U-14) ────────────────────
    /// 일반 보기(서식·이미지를 그대로).
    ViewRich,
    /// 간략 보기(1행 + 작은 썸네일) — 기본값.
    ViewCompact,
    /// 한 줄 보기(평문 1줄).
    ViewPlain,

    // ── 동작 ──────────────────────────────────
    /// 복사(클립보드에 올림).
    ActionCopy,
    /// 평문으로 붙여넣기.
    ActionPastePlain,
    /// 고정 토글.
    ActionPin,
    /// 항목 삭제.
    ActionDelete,
    /// 전체 비우기.
    ActionClearAll,
    /// 설정 열기.
    ActionSettings,
    /// 정보.
    ActionAbout,
    /// 종료.
    ActionQuit,
    /// 메인창 열기.
    ActionOpenMain,
    /// 캡처 일시정지 토글.
    ActionPauseCapture,

    // ── 상태 ──────────────────────────────────
    /// 기록이 암호화되어 있음.
    StatusEncrypted,
    /// 로컬 전용(네트워크로 나가지 않음).
    StatusLocalOnly,
    /// 현재 클립보드 내용(트레이 메뉴 머리).
    StatusCurrentClipboard,
    /// 이 환경에서는 클립보드 수집이 불가능함(Wayland/GNOME 등).
    StatusWatchUnsupported,
}

impl Msg {
    /// `[en, ko, ja]` — 한 줄에 모아 누락을 리뷰에서 잡는다.
    #[must_use]
    pub fn row(self) -> [&'static str; 3] {
        match self {
            Msg::AppName => ["Nexa Clip", "Nexa Clip", "Nexa Clip"],
            Msg::SearchPlaceholder => ["type to search…", "검색어를 입력하세요…", "検索…"],
            Msg::EmptyHistory => [
                "Nothing copied yet",
                "복사한 항목이 없습니다",
                "コピー履歴がありません",
            ],
            Msg::NoMatches => ["No matches", "일치하는 항목 없음", "一致なし"],

            Msg::KindText => ["Text", "텍스트", "テキスト"],
            Msg::KindRichText => ["Formatted text", "서식 있는 텍스트", "書式付きテキスト"],
            Msg::KindImage => ["Image", "이미지", "画像"],
            Msg::KindFiles => ["Files", "파일", "ファイル"],
            Msg::KindColor => ["Color", "색상", "色"],

            Msg::ViewRich => ["Rich", "일반 보기", "リッチ表示"],
            Msg::ViewCompact => ["Compact", "간략 보기", "簡易表示"],
            Msg::ViewPlain => ["Plain", "한 줄 보기", "1行表示"],

            Msg::ActionCopy => ["Copy", "복사", "コピー"],
            Msg::ActionPastePlain => [
                "Paste as plain text",
                "평문으로 붙여넣기",
                "書式なしで貼り付け",
            ],
            Msg::ActionPin => ["Pin", "고정", "ピン留め"],
            Msg::ActionDelete => ["Delete", "삭제", "削除"],
            Msg::ActionClearAll => ["Clear all", "전체 비우기", "すべて消去"],
            Msg::ActionSettings => ["Preferences…", "설정…", "設定…"],
            Msg::ActionAbout => ["About", "정보", "情報"],
            Msg::ActionQuit => ["Quit", "종료", "終了"],
            Msg::ActionOpenMain => ["Open main window…", "메인창 열기…", "メインウィンドウ…"],
            Msg::ActionPauseCapture => ["Pause capturing", "캡처 일시정지", "取り込みを一時停止"],

            Msg::StatusEncrypted => ["Encrypted", "암호화됨", "暗号化済み"],
            Msg::StatusLocalOnly => ["Local only", "로컬 전용", "ローカルのみ"],
            Msg::StatusCurrentClipboard => ["Clipboard", "현재 클립보드", "クリップボード"],
            Msg::StatusWatchUnsupported => [
                "Clipboard capture is not available in this environment",
                "이 환경에서는 클립보드 수집을 할 수 없습니다",
                "この環境ではクリップボードを取得できません",
            ],
        }
    }
}

/// 번역 조회. 빈 문자열이면 **영어로 폴백**한다(누락이 화면을 비우지 않게).
#[must_use]
pub fn tr(lang: Lang, msg: Msg) -> &'static str {
    let row = msg.row();
    let s = match lang {
        Lang::En => row[0],
        Lang::Ko => row[1],
        Lang::Ja => row[2],
    };
    if s.is_empty() {
        row[0]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_roundtrip() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
        assert_eq!(Lang::from_code("xx"), None);
    }

    #[test]
    fn current_lang_follows_set() {
        set_lang(Lang::Ko);
        assert_eq!(current_lang(), Lang::Ko);
        assert_eq!(tr(current_lang(), Msg::ActionCopy), "복사");
        set_lang(Lang::En); // 다른 테스트에 영향 주지 않게 복구
    }

    /// ★ 카탈로그에 빈 칸이 없어야 한다 — 있으면 화면이 비거나 영어로 새어 나간다.
    #[test]
    fn catalog_has_no_empty_cell() {
        let all = [
            Msg::AppName,
            Msg::SearchPlaceholder,
            Msg::EmptyHistory,
            Msg::NoMatches,
            Msg::KindText,
            Msg::KindRichText,
            Msg::KindImage,
            Msg::KindFiles,
            Msg::KindColor,
            Msg::ViewRich,
            Msg::ViewCompact,
            Msg::ViewPlain,
            Msg::ActionCopy,
            Msg::ActionPastePlain,
            Msg::ActionPin,
            Msg::ActionDelete,
            Msg::ActionClearAll,
            Msg::ActionSettings,
            Msg::ActionAbout,
            Msg::ActionQuit,
            Msg::ActionOpenMain,
            Msg::ActionPauseCapture,
            Msg::StatusEncrypted,
            Msg::StatusLocalOnly,
            Msg::StatusCurrentClipboard,
            Msg::StatusWatchUnsupported,
        ];
        for m in all {
            for (i, cell) in m.row().iter().enumerate() {
                assert!(!cell.is_empty(), "{m:?} 의 {i}번 언어가 비어 있다");
            }
        }
    }
}
