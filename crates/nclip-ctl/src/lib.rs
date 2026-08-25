//! `nclip-ctl` — 컨트롤 계층.
//!
//! ## 구성
//!
//! | 출처 | 내용 |
//! |---|---|
//! | `crates/vendor/nbeep-ctl` | ★ **무수정 복사** — 재수출만 한다([docs/13 §2](../../../docs/13-ui-reuse-from-beep.md)) |
//! | 이 크레이트 | ★ 신규 컨트롤 — [`view_mode`] · [`clip_row`] · [`rich_text`] · [`vtoolbar`] |
//!
//! ## ⚠️ 규율 U-1
//!
//! **vendor 안의 파일은 고치지 않는다.** 필요한 확장은 여기서 감싸거나 새로 만든다 —
//! *"복사해 놓고 조금씩 고치기"* 가 가장 나쁜 결말(양쪽이 갈라져 동기화 불가)이라
//! **파일 단위로 경계를 강제**한다.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

// vendor 재수출 — 사용처는 `nclip_ctl::…` 하나만 알면 된다.
pub use nbeep_ctl::{controls, draw, edit, event, geom, raster, theme, widget};

pub mod view_mode;

pub use view_mode::ViewMode;
