// ★ 이식 사본(09-03 · M2 기반) — 원본: nexa-beep crates/nbeep-crypto/src/noise.rs
// ⚠️ 와이어 규약 공유 — beep과 어긋나면 통신이 깨진다(docs/22 I-5 · 변경 시 양쪽 동기).
//! `NoiseSession` — 실물 보안 세션([docs/08] ADR-0002 · DR-11).
//!
//! **`Noise_XX_25519_ChaChaPoly_BLAKE2s`** — 사전 등록 없는(DR-1) 우리 상황에서 양쪽이 핸드셰이크 중
//! 정적 공개키를 교환하는 `XX`가 유일한 선택이다([docs/08] §4). 암호 자체는 직접 구현하지 않고
//! 검증된 [`snow`] 프레임워크에 위임한다(NFR-S-3).
//!
//! **`PeerId` = X25519 정적 공개키**(32바이트) — 핸드셰이크가 상대의 키 소유를 증명하므로 `peer()`는
//! **암호학적으로 인증된** 값이다. 단 **TOFU 핀·SAS 대조는 M2-2** 소관이라, 여기서 `trust()`는
//! 기본 [`TrustLevel::Unverified`]다(핸드셰이크 성립 ≠ 신뢰 확정).

use crate::link::Link;
use crate::session::{Session, SessionError};
use crate::{PeerId, TrustLevel};
use snow::{Builder, HandshakeState, TransportState};

/// Noise 프로토콜 파라미터([docs/08] — X25519 / ChaCha20-Poly1305 / BLAKE2s).
const PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Noise 메시지 상한(프레임워크 제약). 페이로드는 태그(16B)만큼 작아야 한다.
const NOISE_MAX: usize = 65535;
const TAG_LEN: usize = 16;

/// 기기 장기 신원 — X25519 정적 키쌍. **공개키가 곧 [`PeerId`]**(DR-8).
///
/// 개인키는 **로컬에만**(NFR-S-1). 저장·로딩은 `nbeep-store`(M2-5) 소관이며 여기서는 메모리 표현만 든다.
pub struct Identity {
    private: Vec<u8>,
    public: [u8; PeerId::LEN],
}

impl core::fmt::Debug for Identity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // 개인키를 찍지 않는다(docs/13 §7). 공개 지문만.
        f.debug_struct("Identity")
            .field("peer", &self.peer_id())
            .field("private", &"[redacted]")
            .finish()
    }
}

impl Identity {
    /// 새 정적 키쌍을 생성한다(OS 난수 — snow 내부).
    ///
    /// # Panics
    /// Noise 파라미터가 유효하지 않거나 키 생성에 실패하면(사실상 불가) 패닉.
    #[must_use]
    pub fn generate() -> Self {
        let kp = Builder::new(PARAMS.parse().expect("유효한 Noise 파라미터"))
            .generate_keypair()
            .expect("X25519 키 생성");
        let public: [u8; PeerId::LEN] = kp.public.try_into().expect("32바이트 X25519 공개키");
        Self {
            private: kp.private,
            public,
        }
    }

    /// 이 신원의 `PeerId`(= X25519 공개키).
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        PeerId::from_bytes(self.public)
    }

    /// 키 복제 시나리오 재현용(테스트 한정) — 같은 키 자료의 별개 `Identity`.
    #[cfg(test)]
    pub(crate) fn from_parts_for_test(other: &Identity) -> Self {
        Self {
            private: other.private.clone(),
            public: other.public,
        }
    }

    /// 저장용 키 자료(개인 32B ‖ 공개 32B) — [`crate::keyfile`] 전용(M2-5a).
    /// **로그·전송 금지**(NFR-S-1). 공개키를 함께 저장하는 이유: 개인키에서 공개키를
    /// 유도하려면 X25519 스칼라 곱이 필요한데 snow가 노출하지 않는다 — 쌍을 저장하고,
    /// 불일치(변조)는 핸드셰이크 실패로 드러난다(가용성 문제일 뿐 기밀성 문제가 아니다).
    ///
    /// # Panics
    /// 개인키가 32바이트가 아니면(생성 경로상 불가) 패닉.
    #[must_use]
    pub fn key_bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        out[..32].copy_from_slice(&self.private);
        out[32..].copy_from_slice(&self.public);
        out
    }

    /// 저장된 키 자료 복원 — [`Self::key_bytes`]의 역. 파일 무결성(매직·길이)은
    /// [`crate::keyfile`]이 거른다.
    #[must_use]
    pub fn from_key_bytes(bytes: &[u8; 64]) -> Self {
        let mut public = [0u8; PeerId::LEN];
        public.copy_from_slice(&bytes[32..]);
        Self {
            private: bytes[..32].to_vec(),
            public,
        }
    }

    /// 래핑 KDF 원료(개인키 32B — ADR-0005 §3 기본 A "기기 키 파생"). **로그 금지.**
    /// 256비트 무작위 키라 메모리-하드 KDF가 불필요하다(암호가 아니다 — 승격 ②만 해당).
    ///
    /// # Panics
    /// 개인키가 32바이트가 아니면(생성 경로상 불가) 패닉.
    #[must_use]
    pub fn wrap_secret(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.private);
        out
    }
}

fn builder(id: &Identity) -> Result<Builder<'_>, SessionError> {
    let params = PARAMS.parse().map_err(|_| SessionError::Handshake)?;
    Ok(Builder::new(params).local_private_key(&id.private))
}

/// ★ prologue 변형(09-03 clip — 종단 세션 앱 격리 · docs/07 §3-4-2).
/// 서버 제어 세션은 prologue 없는 [`builder`]를 쓴다(beepd 호환).
fn builder_with_prologue<'a>(
    id: &'a Identity,
    prologue: &'a [u8],
) -> Result<Builder<'a>, SessionError> {
    Ok(builder(id)?.prologue(prologue))
}

fn remote_peer(hs: &HandshakeState) -> Result<PeerId, SessionError> {
    let remote = hs.get_remote_static().ok_or(SessionError::Handshake)?;
    let bytes: [u8; PeerId::LEN] = remote.try_into().map_err(|_| SessionError::Handshake)?;
    Ok(PeerId::from_bytes(bytes))
}

/// Noise_XX로 인증·암호화된 세션. `initiate`/`accept`로 수립한다.
pub struct NoiseSession<L: Link> {
    link: L,
    transport: TransportState,
    peer: PeerId,
}

impl<L: Link> core::fmt::Debug for NoiseSession<L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NoiseSession")
            .field("peer", &self.peer)
            .finish()
    }
}

impl<L: Link> NoiseSession<L> {
    /// 개시자 측 핸드셰이크(`-> e` / `<- e,ee,s,es` / `-> s,se`).
    ///
    /// # Errors
    /// 링크 종료·프로토콜 실패 시 [`SessionError`].
    pub fn initiate(link: L, id: &Identity) -> Result<Self, SessionError> {
        Self::initiate_inner(link, id, None)
    }

    /// ★ 종단 세션용 prologue 변형 — 양쪽이 같은 값이 아니면 수학적으로 실패(앱 격리).
    ///
    /// # Errors
    /// 링크 종료·프로토콜 실패 시 [`SessionError`].
    pub fn initiate_with_prologue(
        link: L,
        id: &Identity,
        prologue: &[u8],
    ) -> Result<Self, SessionError> {
        Self::initiate_inner(link, id, Some(prologue))
    }

    fn initiate_inner(
        mut link: L,
        id: &Identity,
        prologue: Option<&[u8]>,
    ) -> Result<Self, SessionError> {
        let b = match prologue {
            Some(p) => builder_with_prologue(id, p)?,
            None => builder(id)?,
        };
        let mut hs = b.build_initiator().map_err(|_| SessionError::Handshake)?;
        let mut buf = vec![0u8; NOISE_MAX];

        let n = hs
            .write_message(&[], &mut buf)
            .map_err(|_| SessionError::Handshake)?;
        link.send(&buf[..n])?;

        let msg = link.recv()?;
        hs.read_message(&msg, &mut buf)
            .map_err(|_| SessionError::Handshake)?;

        let n = hs
            .write_message(&[], &mut buf)
            .map_err(|_| SessionError::Handshake)?;
        link.send(&buf[..n])?;

        Self::finish(link, hs, id)
    }

    /// 수신자 측 핸드셰이크(`<- e` / `-> e,ee,s,es` / `<- s,se`).
    ///
    /// # Errors
    /// 링크 종료·프로토콜 실패 시 [`SessionError`].
    pub fn accept(link: L, id: &Identity) -> Result<Self, SessionError> {
        Self::accept_inner(link, id, None)
    }

    /// ★ 종단 세션용 prologue 변형(수신자 쪽).
    ///
    /// # Errors
    /// 링크 종료·프로토콜 실패 시 [`SessionError`].
    pub fn accept_with_prologue(
        link: L,
        id: &Identity,
        prologue: &[u8],
    ) -> Result<Self, SessionError> {
        Self::accept_inner(link, id, Some(prologue))
    }

    fn accept_inner(
        mut link: L,
        id: &Identity,
        prologue: Option<&[u8]>,
    ) -> Result<Self, SessionError> {
        let b = match prologue {
            Some(p) => builder_with_prologue(id, p)?,
            None => builder(id)?,
        };
        let mut hs = b.build_responder().map_err(|_| SessionError::Handshake)?;
        let mut buf = vec![0u8; NOISE_MAX];

        let msg = link.recv()?;
        hs.read_message(&msg, &mut buf)
            .map_err(|_| SessionError::Handshake)?;

        let n = hs
            .write_message(&[], &mut buf)
            .map_err(|_| SessionError::Handshake)?;
        link.send(&buf[..n])?;

        let msg = link.recv()?;
        hs.read_message(&msg, &mut buf)
            .map_err(|_| SessionError::Handshake)?;

        Self::finish(link, hs, id)
    }

    fn finish(link: L, hs: HandshakeState, id: &Identity) -> Result<Self, SessionError> {
        let peer = remote_peer(&hs)?;
        if peer == id.peer_id() {
            // D-22 U-P2(사용자 확정 08-08): 상대가 내 신원과 같다 — 자기 연결이거나
            // 키 파일 복제다. 즉시 거부 + 경고 대상([docs/21 §5] I-7).
            return Err(SessionError::SelfPeer);
        }
        let transport = hs
            .into_transport_mode()
            .map_err(|_| SessionError::Handshake)?;
        Ok(Self {
            link,
            transport,
            peer,
        })
    }
}

impl<L: Link> Session for NoiseSession<L> {
    fn peer(&self) -> PeerId {
        self.peer
    }

    fn trust(&self) -> TrustLevel {
        // 핸드셰이크는 키를 인증하지만, TOFU 핀·SAS 대조는 M2-2 소관.
        TrustLevel::Unverified
    }

    fn send(&mut self, message: &[u8]) -> Result<(), SessionError> {
        if message.len() > NOISE_MAX - TAG_LEN {
            return Err(SessionError::TooLarge); // 큰 메시지 청킹은 M4(파일 전송)
        }
        let mut out = vec![0u8; message.len() + TAG_LEN];
        let n = self
            .transport
            .write_message(message, &mut out)
            .map_err(|_| SessionError::Closed)?;
        self.link.send(&out[..n])?;
        Ok(())
    }

    fn set_recv_timeout(&mut self, dur: Option<core::time::Duration>) {
        let _ = self.link.set_recv_timeout(dur); // 실패해도 블로킹 폴백(다음 recv에서 드러남)
    }

    fn recv(&mut self) -> Result<Vec<u8>, SessionError> {
        let ciphertext = self.link.recv()?;
        if ciphertext.len() > NOISE_MAX {
            return Err(SessionError::TooLarge);
        }
        let mut out = vec![0u8; ciphertext.len()];
        let n = self
            .transport
            .read_message(&ciphertext, &mut out)
            .map_err(|_| SessionError::Closed)?;
        out.truncate(n);
        Ok(out)
    }
}

// (trust_integration 테스트는 미이식 — trust/trusted 축은 clip DeviceList 설계(09 §6)로 대체 예정)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::duplex;
    use std::thread;

    #[test]
    fn handshake_authenticates_peers_and_encrypts() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let a_id = alice.peer_id();
        let b_id = bob.peer_id();
        let (la, lb) = duplex(a_id, b_id);

        let hb = thread::spawn(move || NoiseSession::accept(lb, &bob));
        let mut a = NoiseSession::initiate(la, &alice).expect("a 수립");
        let mut b = hb.join().unwrap().expect("b 수립");

        // 핸드셰이크가 상대의 정적 공개키(=PeerId)를 인증했다.
        assert_eq!(a.peer(), b_id, "a는 b의 키를 인증");
        assert_eq!(b.peer(), a_id, "b는 a의 키를 인증");

        // 암호화된 왕복.
        a.send(b"secret message").unwrap();
        assert_eq!(b.recv().unwrap(), b"secret message");
        b.send(b"reply").unwrap();
        assert_eq!(a.recv().unwrap(), b"reply");
    }

    #[test]
    fn ciphertext_on_the_wire_is_not_plaintext() {
        // 링크에 실제로 흐르는 바이트가 평문이 아님을 확인 — 별도 fake 링크로 가로채기.
        use crate::link::{Link, LinkError};
        use std::sync::mpsc::{channel, Receiver, Sender};

        // a→b 프레임을 가로채는 링크(테스트 전용).
        struct Tap {
            peer: PeerId,
            tx: Sender<Vec<u8>>,
            rx: Receiver<Vec<u8>>,
            sniff: Sender<Vec<u8>>,
        }
        impl Link for Tap {
            fn peer(&self) -> PeerId {
                self.peer
            }
            fn send(&mut self, f: &[u8]) -> Result<(), LinkError> {
                self.sniff.send(f.to_vec()).ok();
                self.tx.send(f.to_vec()).map_err(|_| LinkError::Closed)
            }
            fn recv(&mut self) -> Result<Vec<u8>, LinkError> {
                self.rx.recv().map_err(|_| LinkError::Closed)
            }
        }

        let alice = Identity::generate();
        let bob = Identity::generate();
        let (a_id, b_id) = (alice.peer_id(), bob.peer_id());
        let (a_tx, a_rx) = channel();
        let (b_tx, b_rx) = channel();
        let (sniff_tx, sniff_rx) = channel();
        let la = Tap {
            peer: b_id,
            tx: a_tx,
            rx: b_rx,
            sniff: sniff_tx,
        };
        let lb = {
            struct Plain {
                peer: PeerId,
                tx: Sender<Vec<u8>>,
                rx: Receiver<Vec<u8>>,
            }
            impl Link for Plain {
                fn peer(&self) -> PeerId {
                    self.peer
                }
                fn send(&mut self, f: &[u8]) -> Result<(), LinkError> {
                    self.tx.send(f.to_vec()).map_err(|_| LinkError::Closed)
                }
                fn recv(&mut self) -> Result<Vec<u8>, LinkError> {
                    self.rx.recv().map_err(|_| LinkError::Closed)
                }
            }
            Plain {
                peer: a_id,
                tx: b_tx,
                rx: a_rx,
            }
        };

        let hb = thread::spawn(move || NoiseSession::accept(lb, &bob));
        let mut a = NoiseSession::initiate(la, &alice).expect("a 수립");
        let mut b = hb.join().unwrap().expect("b 수립");
        // 핸드셰이크 중 흐른 스니핑 프레임을 비운다.
        while sniff_rx.try_recv().is_ok() {}

        a.send(b"topsecret").unwrap();
        assert_eq!(b.recv().unwrap(), b"topsecret");
        let on_wire = sniff_rx.try_recv().expect("전송된 프레임");
        assert!(
            !on_wire.windows(9).any(|w| w == b"topsecret"),
            "평문이 링크에 노출되면 안 된다"
        );
    }

    /// ★ M1-11③ tap 평문 부재의 일반화 — 단일 문자열이 아니라 **금칙어 목록**
    /// (이메일·전화·한글 본문·프로필 설정 키)으로 확장. 세션에 실어 보낸 어떤
    /// 민감 문자열도 링크 바이트에 그대로 나타나면 안 된다(FR-S-51).
    #[test]
    fn wire_never_carries_forbidden_plaintext() {
        use crate::link::{Link, LinkError};
        use std::sync::mpsc::{channel, Receiver, Sender};
        struct Tap {
            peer: PeerId,
            tx: Sender<Vec<u8>>,
            rx: Receiver<Vec<u8>>,
            sniff: Sender<Vec<u8>>,
        }
        impl Link for Tap {
            fn peer(&self) -> PeerId {
                self.peer
            }
            fn send(&mut self, f: &[u8]) -> Result<(), LinkError> {
                self.sniff.send(f.to_vec()).ok();
                self.tx.send(f.to_vec()).map_err(|_| LinkError::Closed)
            }
            fn recv(&mut self) -> Result<Vec<u8>, LinkError> {
                self.rx.recv().map_err(|_| LinkError::Closed)
            }
        }
        struct Plain {
            peer: PeerId,
            tx: Sender<Vec<u8>>,
            rx: Receiver<Vec<u8>>,
        }
        impl Link for Plain {
            fn peer(&self) -> PeerId {
                self.peer
            }
            fn send(&mut self, f: &[u8]) -> Result<(), LinkError> {
                self.tx.send(f.to_vec()).map_err(|_| LinkError::Closed)
            }
            fn recv(&mut self) -> Result<Vec<u8>, LinkError> {
                self.rx.recv().map_err(|_| LinkError::Closed)
            }
        }
        let alice = Identity::generate();
        let bob = Identity::generate();
        let (a_id, b_id) = (alice.peer_id(), bob.peer_id());
        let (a_tx, a_rx) = channel();
        let (b_tx, b_rx) = channel();
        let (sniff_tx, sniff_rx) = channel();
        let la = Tap {
            peer: b_id,
            tx: a_tx,
            rx: b_rx,
            sniff: sniff_tx,
        };
        let lb = Plain {
            peer: a_id,
            tx: b_tx,
            rx: a_rx,
        };
        let hb = thread::spawn(move || NoiseSession::accept(lb, &bob));
        let mut a = NoiseSession::initiate(la, &alice).expect("a 수립");
        let mut b = hb.join().unwrap().expect("b 수립");
        while sniff_rx.try_recv().is_ok() {}

        // 금칙어 세트 — 프로필·연락처·한글 본문(29 §4-1 취지: 사람이 읽을 것).
        const FORBIDDEN: &[&[u8]] = &[
            b"kiros33@gmail.com",
            b"010-1234-5678",
            b"profile.share.email",
            "홍길동".as_bytes(),
            "비밀 회의는 3시".as_bytes(),
        ];
        for f in FORBIDDEN {
            a.send(f).unwrap();
            assert_eq!(&b.recv().unwrap(), f, "복호는 온전");
        }
        let mut wire = Vec::new();
        while let Ok(fr) = sniff_rx.try_recv() {
            wire.extend_from_slice(&fr);
        }
        assert!(!wire.is_empty());
        for f in FORBIDDEN {
            assert!(
                !wire.windows(f.len()).any(|w| w == *f),
                "링크 바이트에 금칙어 노출: {:?}",
                String::from_utf8_lossy(f)
            );
        }
    }

    /// ★ M1-11④ Debug 마스킹 — Identity의 Debug 출력에 개인키 바이트가 새지
    /// 않는다(docs/13 §7 · 로그는 상태만). 지문(공개)만 허용.
    #[test]
    fn identity_debug_never_leaks_private_key() {
        let id = Identity::generate();
        let dbg = format!("{id:?}");
        // 개인키의 어떤 4바이트 연속도 hex로 나타나지 않아야 한다(전수 검사 —
        // 2바이트는 공개 지문과 우연히 겹칠 확률이 있어 4바이트로 판정).
        for w in id.private.windows(4) {
            let hex_lower = format!("{:02x}{:02x}{:02x}{:02x}", w[0], w[1], w[2], w[3]);
            let hex_upper = hex_lower.to_uppercase();
            assert!(
                !dbg.contains(&hex_lower) && !dbg.contains(&hex_upper),
                "Debug에 개인키 hex 조각: {dbg}"
            );
        }
        assert!(dbg.contains("Identity"), "형식 확인: {dbg}");
    }

    #[test]
    fn distinct_identities_have_distinct_peer_ids() {
        assert_ne!(
            Identity::generate().peer_id(),
            Identity::generate().peer_id()
        );
    }
}
