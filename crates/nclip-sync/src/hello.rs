//! ★ 종단 인사 프레임(09-03 — 기기 이름 교환). Noise 세션 **안**의 논리 메시지라 서버는
//! 봉투만 본다(DR-4). beep 서버·와이어 무관 — docs/22 대상 아님.
//!
//! ```text
//! Hello = "NCH1" ‖ name_len u8 ‖ name(utf8) ‖ os_len u8 ‖ os(utf8)
//! Ping  = "NCP1"        Pong = "NCQ1"
//! ```
//! 미지 태그는 조용히 버린다(전방 호환).

use crate::name::DisplayName;

const TAG_HELLO: &[u8; 4] = b"NCH1";
const TAG_PING: &[u8; 4] = b"NCP1";
const TAG_PONG: &[u8; 4] = b"NCQ1";

/// 세션 첫 메시지 — 내가 누구로 보이고 싶은가(신원은 세션이 이미 확정했다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    /// 표시 이름(무해화됨).
    pub name: DisplayName,
    /// OS 태그(`windows`/`macos`/`linux`) — 목록 보조 표시.
    pub os: String,
}

/// 세션 논리 메시지.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerMsg {
    Hello(Hello),
    Ping,
    Pong,
}

impl PeerMsg {
    /// 인코딩.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            PeerMsg::Hello(h) => {
                let clamped = clamp_name(&h.name);
                let name = clamped.as_str().as_bytes();
                let os = h.os.as_bytes();
                let os = &os[..os.len().min(32)];
                let mut v = Vec::with_capacity(6 + name.len() + os.len());
                v.extend_from_slice(TAG_HELLO);
                v.push(name.len() as u8); // clamp_name이 ≤ 255B를 보장
                v.extend_from_slice(name);
                v.push(os.len() as u8);
                v.extend_from_slice(os);
                v
            }
            PeerMsg::Ping => TAG_PING.to_vec(),
            PeerMsg::Pong => TAG_PONG.to_vec(),
        }
    }

    /// 디코딩 — 형식 위반·미지 태그는 `None`.
    #[must_use]
    pub fn decode(b: &[u8]) -> Option<Self> {
        if b.len() < 4 {
            return None;
        }
        let (tag, rest) = b.split_at(4);
        match tag {
            t if t == TAG_PING => Some(PeerMsg::Ping),
            t if t == TAG_PONG => Some(PeerMsg::Pong),
            t if t == TAG_HELLO => {
                let (n, rest) = rest.split_first()?;
                let (name, rest) = rest.split_at_checked(usize::from(*n))?;
                let (m, rest) = rest.split_first()?;
                let (os, _) = rest.split_at_checked(usize::from(*m))?;
                let name = DisplayName::parse(std::str::from_utf8(name).ok()?).ok()?;
                let os = std::str::from_utf8(os).ok()?.to_string();
                Some(PeerMsg::Hello(Hello { name, os }))
            }
            _ => None,
        }
    }
}

impl Hello {
    /// 이 기기의 인사 — 이름은 호출자가 정한다(설정 `sync.device_name` 또는 기본 이름).
    #[must_use]
    pub fn local(name: DisplayName) -> Self {
        Self {
            name,
            os: std::env::consts::OS.to_string(),
        }
    }
}

/// 이름 바이트 상한 — u8 길이 필드(utf8 64자는 256B를 넘을 수 있어 잘라 보낸다).
pub const MAX_NAME_BYTES: usize = 255;

/// 이름을 바이트 상한에 맞춰 자른 표시 이름(char 경계).
#[must_use]
pub fn clamp_name(name: &DisplayName) -> DisplayName {
    let s = name.as_str();
    if s.len() <= MAX_NAME_BYTES {
        return name.clone();
    }
    let mut end = MAX_NAME_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    DisplayName::parse(&s[..end]).unwrap_or_else(|_| name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrip() {
        let h = Hello {
            name: DisplayName::parse("작업용 PC").unwrap(),
            os: "windows".into(),
        };
        let m = PeerMsg::Hello(h.clone());
        assert_eq!(PeerMsg::decode(&m.encode()), Some(PeerMsg::Hello(h)));
        assert_eq!(
            PeerMsg::decode(&PeerMsg::Ping.encode()),
            Some(PeerMsg::Ping)
        );
        assert_eq!(
            PeerMsg::decode(&PeerMsg::Pong.encode()),
            Some(PeerMsg::Pong)
        );
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(PeerMsg::decode(b"XXXX"), None);
        assert_eq!(PeerMsg::decode(b"NCH1\x05ab"), None, "길이 초과 = 위반");
        assert_eq!(PeerMsg::decode(b""), None);
    }

    #[test]
    fn long_name_is_clamped_to_byte_cap() {
        let raw = "가".repeat(64); // 192B — 상한 안
        let n = DisplayName::parse(&raw).unwrap();
        assert_eq!(clamp_name(&n).as_str().len(), 192);
        let e = PeerMsg::Hello(Hello {
            name: n,
            os: "linux".into(),
        })
        .encode();
        assert!(PeerMsg::decode(&e).is_some());
    }
}
