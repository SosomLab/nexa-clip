//! ★ M2 동기화 기반(09-03 사용자 요청) — 신원·랑데부 파생·릴레이 접속의 뿌리.
//!
//! 설계 SSOT: [docs/07 랑데부](../../docs/07-device-rendezvous.md) ·
//! [docs/09 신원·페어링](../../docs/09-identity-and-pairing.md).
//! 서버는 beep의 `nexa-beepd`를 **변경 없이** 공유한다([docs/22 원장](../../docs/22-upstream-beep-liaison.md) I-1~I-5).

pub mod rid;

pub use rid::{current_epoch_day, derive_rid, rids_around, Rid};
