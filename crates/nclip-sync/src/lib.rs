//! ★ M2 동기화 기반(09-03 사용자 요청) — 신원·랑데부·릴레이 클라이언트·홀펀칭.
//!
//! 설계 SSOT: [docs/07 랑데부](../../docs/07-device-rendezvous.md) ·
//! [docs/09 신원·페어링](../../docs/09-identity-and-pairing.md).
//! 서버는 beep의 `nexa-beepd`를 **변경 없이** 공유한다([docs/22](../../docs/22-upstream-beep-liaison.md)).
//!
//! ## 이식 계보(사본 · docs/22 I-5)
//! `nbeep-core(identity/link/session/path/endpoint)` · `nbeep-crypto(noise/keyfile/sas)` ·
//! `nbeep-net(tcp/udplink/arq)` · `nbeep-relay(lib → relay)` — 와이어 규약은 beep과
//! 공유된다: kind 상수·OBS_MAGIC·포트는 **절대 변경 금지**. 앱 격리 지점만 다르다:
//! 랑데부 도메인 `nclip-rid-v1`(relay) · 종단 prologue `nexa-clip/1`(noise) · 키파일 매직 `NCK1`.

// ★ 이식 사본 완화(09-03) — beep 린트 기준으로 쓰인 코드: 원본 diff를 최소로
//   유지하기 위해 테스트 unwrap·미사용 헬퍼만 크레이트 단위로 허용한다(동기 비용 ↓).
#![allow(clippy::unwrap_used, dead_code)]

pub mod arq;
pub mod endpoint;
/// ★ 종단 인사 프레임(09-03 — 기기 이름 교환).
pub mod hello;
pub mod identity;
pub mod keyfile;
pub mod link;
/// ★ 표시 이름 무해화(09-03 — beep name.rs 이식).
pub mod name;
pub mod noise;
pub mod path;
pub mod relay;
pub mod rid;
pub mod sas;
pub mod session;
pub mod tcp;
/// 테스트 전용 가짜 링크(duplex) — 이식 사본.
#[cfg(test)]
pub mod testkit;
pub mod udplink;

/// 계측 스텁 — beep netmon(트래픽 카운터)의 호출 자리만 메운다(이식 최소 차분).
pub mod netmon {
    /// 세션 송신 계측(no-op).
    pub fn on_sess_tx(_bytes: u64) {}
    /// 세션 수신 계측(no-op).
    pub fn on_sess_rx(_bytes: u64) {}
}

pub use identity::{DeviceId, PeerId, Recipients, TrustLevel, UserId};
pub use link::Link;
pub use noise::Identity;
pub use noise::NoiseSession;
pub use path::{class_of_ip, PathClass};
pub use relay::{attach, resolve_server, AttachError, Attached, RelayClient};
pub use rid::{current_epoch_day, derive_rid, rids_around, Rid as PairRid};
pub use session::{Session, SessionError};
pub use tcp::TcpLink;
pub use udplink::UdpLink;
