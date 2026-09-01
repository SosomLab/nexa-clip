//! `nclip-ui` — 화면. [`nclip_core`] 상태를 읽어 [`nclip_ctl`]로 그린다.
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

pub mod settings;
mod settings_registry;

pub mod hangul;
pub mod typeahead;
pub use settings::{registry, Entry, NoteTone, SettingKind, SettingsState, SettingsWidget};
