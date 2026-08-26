//! ★ **설정 항목 레지스트리** — [`crate::settings`] 프레임워크가 읽는 **단일 원천**.
//!
//! [docs/13 §2-3](../../../docs/13-ui-reuse-from-beep.md)에서 *"`registry()`만 교체하면
//! 설정 화면이 그대로 산다"* 고 했던 **그 교체 데이터**다. 화면 코드는 한 줄도 안 고쳤다.
//!
//! ## 이 구조가 주는 것
//!
//! 렌더와 **설정 검색**이 같은 원천을 읽으므로 *"화면에 있는데 검색 안 되는 설정"* 이
//! **구조적으로 불가능**하다. 새 설정 = 여기 한 줄 + [`nclip_core::Msg`] 키 한 줄.
//!
//! ## 범위
//!
//! 1차는 **동작하는 최소 집합**이다 — [docs/14 §3](../../../docs/14-settings-registry.md)의
//! 전체 명세 중 **지금 구현된 기능**에 해당하는 항목만 넣었다.
//! 캡처·동기화 실물이 붙으면 그때 채운다(빈 설정을 먼저 그리지 않는다).

use crate::settings::{Entry, SettingKind};
use nclip_core::Msg;

/// 언어 후보 — 4개 기본 관리([DR-21](../../../docs/10-decision-record.md)).
const LANG_OPTS: &[(&str, Msg)] = &[
    ("en", Msg::ValLangEn),
    ("ko", Msg::ValLangKo),
    ("zh", Msg::ValLangZh),
    ("ja", Msg::ValLangJa),
];

/// 팝업 위치 — ★ 기본값 **커서**([DR-24](../../../docs/10-decision-record.md) 필수 기능).
const POPUP_AT_OPTS: &[(&str, Msg)] = &[
    ("cursor", Msg::ValCursor),
    ("center", Msg::ValScreenCenter),
    ("last", Msg::ValLastPos),
];

/// 목록 보기 모드(FR-U-14).
const VIEW_OPTS: &[(&str, Msg)] = &[
    ("rich", Msg::ViewRich),
    ("compact", Msg::ViewCompact),
    ("plain", Msg::ViewPlain),
];

/// 테마.
const THEME_OPTS: &[(&str, Msg)] = &[
    ("system", Msg::ValSystem),
    ("dark", Msg::ValDark),
    ("light", Msg::ValLight),
];

/// 정렬 축 — Maccy 선례(최근 / 최초 / 횟수).
const SORT_OPTS: &[(&str, Msg)] = &[
    ("recent", Msg::ValRecentCopy),
    ("first", Msg::ValFirstCopy),
    ("count", Msg::ValCopyCount),
];

/// 검색 방식 — 설정 택일 + 메인창 토글 **둘 다**(D-39).
const SEARCH_OPTS: &[(&str, Msg)] = &[
    ("exact", Msg::ValExact),
    ("fuzzy", Msg::ValFuzzy),
    ("regex", Msg::ValRegex),
];

/// 보관 개수 후보 — 직접 입력 허용(Maccy는 200, 우리 기본 1000).
const MAX_ITEMS_OPTS: &[(&str, Msg)] = &[
    ("200", Msg::SizeSmall),
    ("1000", Msg::SizeNormal),
    ("5000", Msg::SizeLarge),
    ("10000", Msg::SizeExtraLarge),
];

/// 트레이 최근 항목 수(5~10 — [docs/04 TR-1](../../../docs/04-feature-scope-and-screens.md)).
const TRAY_N_OPTS: &[(&str, Msg)] = &[
    ("5", Msg::SizeSmall),
    ("8", Msg::SizeNormal),
    ("10", Msg::SizeLarge),
];

/// 항목 하나 — 반복을 줄인다(`sub`는 아직 안 쓴다).
const fn e(cat: Msg, label: Msg, desc: Msg, kind: SettingKind, key: &'static str) -> Entry {
    Entry {
        cat,
        label,
        desc,
        sub: None,
        kind,
        key,
    }
}

/// ★ 레지스트리 — 렌더·검색·기본값이 전부 여기서 나온다.
pub(crate) const REGISTRY: &[Entry] = &[
    // ── 일반 ────────────────────────────────────────────────
    e(
        Msg::CatGeneral,
        Msg::SetAutostart,
        Msg::SetAutostartDesc,
        SettingKind::Toggle,
        "app.autostart",
    ),
    e(
        Msg::CatGeneral,
        Msg::SetLang,
        Msg::SetLangDesc,
        SettingKind::Radio(LANG_OPTS),
        "app.lang",
    ),
    // ── 캡처 ────────────────────────────────────────────────
    e(
        Msg::CatCapture,
        Msg::SetCapText,
        Msg::SetCapText,
        SettingKind::Toggle,
        "cap.text",
    ),
    e(
        Msg::CatCapture,
        Msg::SetCapImage,
        Msg::SetCapImage,
        SettingKind::Toggle,
        "cap.image",
    ),
    e(
        Msg::CatCapture,
        Msg::SetCapFiles,
        Msg::SetCapFiles,
        SettingKind::Toggle,
        "cap.files",
    ),
    e(
        Msg::CatCapture,
        Msg::SetCapRich,
        Msg::SetCapRich,
        SettingKind::Toggle,
        "cap.rich",
    ),
    e(
        Msg::CatCapture,
        Msg::SetCapNative,
        Msg::SetCapNativeDesc,
        SettingKind::Toggle,
        "cap.native_formats",
    ),
    // ── 보관 ────────────────────────────────────────────────
    e(
        Msg::CatStorage,
        Msg::SetMaxItems,
        Msg::SetMaxItemsDesc,
        SettingKind::RadioInput(MAX_ITEMS_OPTS, ""),
        "store.max_items",
    ),
    e(
        Msg::CatStorage,
        Msg::SetSortBy,
        Msg::SetSortBy,
        SettingKind::Radio(SORT_OPTS),
        "store.sort",
    ),
    // ── 보안·개인정보 ───────────────────────────────────────
    e(
        Msg::CatPrivacy,
        Msg::SetRespectMarks,
        Msg::SetRespectMarksDesc,
        SettingKind::Toggle,
        "sec.respect_marks",
    ),
    e(
        Msg::CatPrivacy,
        Msg::SetClearOnQuit,
        Msg::SetClearOnQuit,
        SettingKind::Toggle,
        "sec.clear_on_quit",
    ),
    // ── 붙여넣기 ────────────────────────────────────────────
    e(
        Msg::CatPaste,
        Msg::SetPasteAuto,
        Msg::SetPasteAutoDesc,
        SettingKind::Toggle,
        "paste.auto",
    ),
    e(
        Msg::CatPaste,
        Msg::SetPastePlainDefault,
        Msg::SetPastePlainDefault,
        SettingKind::Toggle,
        "paste.plain_default",
    ),
    // ── 모양 ────────────────────────────────────────────────
    e(
        Msg::CatAppearance,
        Msg::SetPopupAt,
        Msg::SetPopupAtDesc,
        SettingKind::Radio(POPUP_AT_OPTS),
        "ui.popup_at",
    ),
    e(
        Msg::CatAppearance,
        Msg::SetViewMode,
        Msg::SetViewModeDesc,
        SettingKind::Radio(VIEW_OPTS),
        "ui.view_mode",
    ),
    e(
        Msg::CatAppearance,
        Msg::SetTheme,
        Msg::SetThemeDesc,
        SettingKind::Radio(THEME_OPTS),
        "ui.theme",
    ),
    e(
        Msg::CatAppearance,
        Msg::SetTrayRecent,
        Msg::SetTrayRecent,
        SettingKind::Radio(TRAY_N_OPTS),
        "ui.tray_recent_n",
    ),
    // ── 검색 ────────────────────────────────────────────────
    e(
        Msg::CatSearch,
        Msg::SetSearchMode,
        Msg::SetSearchMode,
        SettingKind::Radio(SEARCH_OPTS),
        "find.mode",
    ),
    e(
        Msg::CatSearch,
        Msg::SetHangulCompose,
        Msg::SetHangulCompose,
        SettingKind::Toggle,
        "find.hangul_compose",
    ),
    // ── 동기화 ──────────────────────────────────────────────
    e(
        Msg::CatSync,
        Msg::SyncEnabled,
        Msg::SetSyncEnabledDesc,
        SettingKind::Toggle,
        "sync.enabled",
    ),
    // ── 고급 ────────────────────────────────────────────────
    e(
        Msg::CatAdvanced,
        Msg::SetDiagLog,
        Msg::SetDiagLogDesc,
        SettingKind::Toggle,
        "adv.log",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry;

    /// ★ 값 키가 겹치면 **한 설정이 다른 설정을 덮어쓴다** — 조용히 깨지므로 여기서 막는다.
    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in REGISTRY {
            assert!(seen.insert(e.key), "값 키 중복: {}", e.key);
        }
    }

    /// 모든 항목이 기본값을 낸다 — 없으면 첫 실행에서 빈 값이 화면에 뜬다.
    #[test]
    fn every_entry_has_defaults() {
        for e in REGISTRY {
            assert!(!e.default_values().is_empty(), "기본값 없음: {}", e.key);
        }
    }

    /// 프레임워크가 우리 레지스트리를 본다(스텁이 남아 있지 않다).
    #[test]
    fn framework_sees_our_registry() {
        assert_eq!(registry().len(), REGISTRY.len());
        assert!(registry().iter().any(|e| e.key == "ui.popup_at"));
    }

    /// ★ 팝업 위치 기본값은 **커서**여야 한다(DR-24 — 사용자 확정 필수 기능).
    #[test]
    fn popup_defaults_to_cursor() {
        let e = REGISTRY
            .iter()
            .find(|e| e.key == "ui.popup_at")
            .expect("ui.popup_at 항목이 있어야 한다");
        let v = e.default_values();
        assert_eq!(v.first().map(|(_, v)| v.as_str()), Some("cursor"));
    }

    /// 평문 붙여넣기 항목이 존재한다(DR-24 필수 기능 ②).
    #[test]
    fn plain_paste_setting_exists() {
        assert!(REGISTRY.iter().any(|e| e.key == "paste.plain_default"));
    }
}
