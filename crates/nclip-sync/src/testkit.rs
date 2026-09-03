// ★ 이식 사본(축약 · 09-03) — 원본: nexa-beep crates/nbeep-core/src/testkit.rs
//   duplex 링크만 가져왔다(action/pipeline/ports 축은 beep 도메인 — clip 미사용).
//! 테스트 전용 인메모리 링크 — 네트워크 없이 세션·프로토콜을 검증한다.

use crate::identity::PeerId;
use crate::link::{Link, LinkError};
use std::sync::mpsc::{channel, Receiver, Sender};

/// 채널로 이어진 인메모리 링크(바이트 관). [`duplex`]가 만든다.
#[derive(Debug)]
pub struct DuplexLink {
    peer: PeerId,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

impl Link for DuplexLink {
    fn peer(&self) -> PeerId {
        self.peer
    }
    fn send(&mut self, frame: &[u8]) -> Result<(), LinkError> {
        self.tx.send(frame.to_vec()).map_err(|_| LinkError::Closed)
    }
    fn recv(&mut self) -> Result<Vec<u8>, LinkError> {
        self.rx.recv().map_err(|_| LinkError::Closed)
    }
}

/// 서로 연결된 링크 한 쌍 — `a`는 `b_peer`를, `b`는 `a_peer`를 향한다.
#[must_use]
pub fn duplex(a_peer: PeerId, b_peer: PeerId) -> (DuplexLink, DuplexLink) {
    let (a_tx, a_rx) = channel::<Vec<u8>>();
    let (b_tx, b_rx) = channel::<Vec<u8>>();
    (
        DuplexLink {
            peer: b_peer,
            tx: a_tx,
            rx: b_rx,
        },
        DuplexLink {
            peer: a_peer,
            tx: b_tx,
            rx: a_rx,
        },
    )
}
