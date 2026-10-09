//! `nclip-ui` — 화면. [`nclip_core`] 상태를 읽어 [`nexa_ctl`]로 그린다.
//!
//! 플랫폼 API를 직접 부르지 않는다(그건 `nclip-plat`의 일이다).
//!
//! ## 화면 목록 ([docs/04 §2](../../../docs/04-feature-scope-and-screens.md))
//!
//! | 화면 | 성격 |
//! |---|---|
//! | S1 퀵 팝업 | 헤더 1줄 + 목록 + 푸터 1줄 — **가장 빠른 경로** |
//! | S2 메인창 | 메뉴+검색 1줄 · **좌측 세로 툴바 40px** · 목록(세로 최대) |
//! | S3 설정 | ♻ beep `settings.rs` 이식 — **`registry()`만 교체**한다 |
//! | S6 트레이 메뉴 | 최근 N개 + 현재 클립보드 + 평문 붙여넣기 |
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

/// 한글 자모열 검색(10-10 · 설정 검색 — nexa-beep jamo.rs 이식).
pub mod jamo;
/// 라이선스 화면 위젯(10-10 P4 · beep license_win.rs 이식).
pub mod license_win;
/// 파일 선택 대화상자 조립(10-10 P3 · nexa-dlg FilePicker + 앱 라벨·필터 · beep picker_win.rs 이식).
pub mod picker_win;
pub mod settings;
mod settings_registry;

/// 한글 2벌식 직접 조합기 — 10-10 nexa-ctl로 이관(본문 동일 · 사본 삭제).
pub use nexa_ctl::hangul;
pub mod typeahead;
pub use settings::{registry, Entry, NoteTone, SettingKind, SettingsState, SettingsWidget};

/// ★ 시험 전용 — 전역 언어(`nclip_core::set_lang`)를 바꾸거나 `current_lang()`에 기대는 시험은 이 잠금을 쥔다
/// (10-10 CI ubuntu: 피커 라벨 시험이 4개 국어를 돌리는 사이 라이선스 시험이 문자열을 비교해 zh/ja로 어긋남 ·
/// 시험은 병렬이라 프로세스 전역 스위치는 직렬화해야 한다 — nexa-ui `GdiOn` 가드와 같은 규칙).
#[cfg(test)]
pub(crate) fn lang_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
