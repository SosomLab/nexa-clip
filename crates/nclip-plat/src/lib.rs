//! `nclip-plat` — 플랫폼 경계. **OS 의존은 전부 여기 하나에 격리**된다.
//!
//! [`nclip_core`]가 선언한 포트를 OS별로 구현한다. 나머지 크레이트는 3타깃 공통이다.
//!
//! ## ⚠️ 이 프로젝트 난이도가 여기 모인다
//!
//! | 항목 | 상태 | 근거 |
//! |---|---|---|
//! | **클립보드 감시** | 🆕 신규 — beep에는 읽기/쓰기만 있고 **watch가 없다** | [docs/20 §3-1](../../../docs/20-implementation-spec.md) |
//! | **전역 단축키** | 🆕 신규 | [docs/20 §3-3](../../../docs/20-implementation-spec.md) |
//! | **직전 포커스 창 복원 + 키 주입** | 🆕 신규 · ★ **K-1 리스크** | [docs/02 §7](../../../docs/02-roadmap.md) |
//! | 클립보드 읽기/쓰기 · 트레이 · 자동시작 | ♻ beep 이식 예정 | [docs/13 §2-5](../../../docs/13-ui-reuse-from-beep.md) |
#![forbid(unsafe_op_in_unsafe_fn)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod autostart;
pub mod clipboard;
pub mod font;
pub mod imgdec;
pub mod paste;
pub mod paths;
pub mod tray;
pub mod watch;
#[cfg(windows)]
pub mod watch_win;
#[cfg(windows)]
pub(crate) mod win32;
