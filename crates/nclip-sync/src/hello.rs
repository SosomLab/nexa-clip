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
/// ★ 클립보드 항목 조각(09-04) — 페이로드는 앱의 휴대 형식 · Noise 64KB 한계로 청킹.
const TAG_ITEM: &[u8; 4] = b"NCI1";
/// 조각 크기(Noise 상한 65535 − 태그·헤더·MAC 여유).
pub const CHUNK: usize = 60_000;
/// 조립 상한(조각 수 × CHUNK) — 이보다 큰 항목은 받지 않는다.
pub const MAX_ITEM: usize = 32 * 1024 * 1024;

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
    /// 클립보드 항목 조각 — `seq`로 묶고 `idx/total`로 맞춘다.
    Item {
        seq: u32,
        idx: u16,
        total: u16,
        data: Vec<u8>,
    },
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
            PeerMsg::Item {
                seq,
                idx,
                total,
                data,
            } => {
                let mut v = Vec::with_capacity(12 + data.len());
                v.extend_from_slice(TAG_ITEM);
                v.extend_from_slice(&seq.to_le_bytes());
                v.extend_from_slice(&idx.to_le_bytes());
                v.extend_from_slice(&total.to_le_bytes());
                v.extend_from_slice(data);
                v
            }
        }
    }

    /// 페이로드를 조각 프레임들로(전송 순서대로).
    #[must_use]
    pub fn chunks(seq: u32, payload: &[u8]) -> Vec<PeerMsg> {
        let total = payload
            .len()
            .div_ceil(CHUNK)
            .max(1)
            .min(usize::from(u16::MAX));
        (0..total)
            .map(|i| PeerMsg::Item {
                seq,
                idx: i as u16,
                total: total as u16,
                data: payload[i * CHUNK..((i + 1) * CHUNK).min(payload.len())].to_vec(),
            })
            .collect()
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
            t if t == TAG_ITEM => {
                let (seq, rest) = rest.split_at_checked(4)?;
                let (idx, rest) = rest.split_at_checked(2)?;
                let (total, data) = rest.split_at_checked(2)?;
                Some(PeerMsg::Item {
                    seq: u32::from_le_bytes(seq.try_into().ok()?),
                    idx: u16::from_le_bytes(idx.try_into().ok()?),
                    total: u16::from_le_bytes(total.try_into().ok()?),
                    data: data.to_vec(),
                })
            }
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

/// 조각 조립기 — `seq`별로 모아 완성되면 돌려준다(상한 초과·중복은 버림).
/// 조립 중인 항목 하나 — (조각 수, 조각들).
type Pending = (u16, Vec<Option<Vec<u8>>>);

#[derive(Default, Debug)]
pub struct Assembler {
    parts: std::collections::HashMap<u32, Pending>,
}

impl Assembler {
    /// 조각 하나 — 완성되면 `Some(페이로드)`.
    pub fn push(&mut self, seq: u32, idx: u16, total: u16, data: Vec<u8>) -> Option<Vec<u8>> {
        if total == 0 || idx >= total || usize::from(total) * CHUNK > MAX_ITEM {
            return None;
        }
        if self.parts.len() > 8 {
            self.parts.clear(); // 미완 잔재 폭주 방지(정상 흐름은 한 번에 하나)
        }
        let e = self
            .parts
            .entry(seq)
            .or_insert_with(|| (total, vec![None; usize::from(total)]));
        if e.0 != total {
            return None;
        }
        e.1[usize::from(idx)] = Some(data);
        if e.1.iter().all(Option::is_some) {
            let (_, v) = self.parts.remove(&seq)?;
            Some(v.into_iter().flatten().flatten().collect())
        } else {
            None
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
    fn item_chunks_roundtrip_through_assembler() {
        let payload: Vec<u8> = (0..150_000u32).map(|i| i as u8).collect();
        let msgs = PeerMsg::chunks(7, &payload);
        assert_eq!(msgs.len(), 3);
        let mut asm = Assembler::default();
        let mut out = None;
        for m in msgs {
            let Some(PeerMsg::Item {
                seq,
                idx,
                total,
                data,
            }) = PeerMsg::decode(&m.encode())
            else {
                panic!("decode")
            };
            out = asm.push(seq, idx, total, data);
        }
        assert_eq!(out.as_deref(), Some(payload.as_slice()));
        assert!(asm.push(9, 0, 1, vec![1]).is_some(), "단일 조각");
        assert!(asm.push(9, 5, 1, vec![1]).is_none(), "idx ≥ total = 위반");
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
