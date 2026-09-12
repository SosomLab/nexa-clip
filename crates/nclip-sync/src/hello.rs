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
/// ★ 파일 내용 공유(09-12 · DR-30 · docs/26) — **당겨 받기(pull)** 5종. 받는 쪽이 블록 단위로
/// `FileReq{offset,len}`를 보내고, 보내는 쪽이 `FileData`×n 뒤 `FileEnd`로 답한다.
/// 흐름 제어·이어 받기·저속 캐싱이 전부 "다음 블록을 언제 요청하느냐"로 귀결된다.
///
/// ```text
/// FileReq    = "NCF1" ‖ req u32 ‖ offset u64 ‖ len u32 ‖ path_len u16 ‖ path(utf8)
/// FileData   = "NCF2" ‖ req u32 ‖ offset u64 ‖ data
/// FileEnd    = "NCF3" ‖ req u32 ‖ size u64 ‖ mtime u64      (블록 끝 · 실제 크기/수정시각)
/// FileErr    = "NCF4" ‖ req u32 ‖ code u8 ‖ msg(utf8)
/// FileCancel = "NCF5" ‖ req u32
/// ```
const TAG_FREQ: &[u8; 4] = b"NCF1";
const TAG_FDAT: &[u8; 4] = b"NCF2";
const TAG_FEND: &[u8; 4] = b"NCF3";
const TAG_FERR: &[u8; 4] = b"NCF4";
const TAG_FCAN: &[u8; 4] = b"NCF5";
/// 파일 데이터 한 프레임의 상한(= [`CHUNK`]) — 블록은 이 프레임 여러 개로 흐른다.
pub const FILE_FRAME: usize = CHUNK;
/// 한 요청이 받을 수 있는 블록 상한(8 MiB) — 받는 쪽 메모리·시간 예산.
pub const FILE_BLOCK_MAX: u32 = 8 * 1024 * 1024;
/// 경로 길이 상한(u16 필드 · 실경로는 훨씬 짧다).
pub const FILE_PATH_MAX: usize = 4096;

/// [`PeerMsg::FileErr`] 사유 — 미지 값은 [`FileErrCode::Other`](전방 호환).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileErrCode {
    /// 그 경로를 제안한 적이 없다(내 파일 항목에 없는 경로 — 임의 읽기 차단).
    NotOffered,
    /// 파일이 없거나 열 수 없다.
    NotFound,
    /// 읽기 오류.
    Io,
    /// 보내는 쪽이 파일 내용 공유를 껐다.
    Disabled,
    /// 기타.
    Other,
}

impl FileErrCode {
    fn to_byte(self) -> u8 {
        match self {
            Self::NotOffered => 1,
            Self::NotFound => 2,
            Self::Io => 3,
            Self::Disabled => 4,
            Self::Other => 0,
        }
    }
    fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::NotOffered,
            2 => Self::NotFound,
            3 => Self::Io,
            4 => Self::Disabled,
            _ => Self::Other,
        }
    }
}

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
    /// ★ 파일 블록 요청(받는 쪽 → 보내는 쪽) — `offset`부터 `len` 바이트.
    FileReq {
        req: u32,
        offset: u64,
        len: u32,
        path: String,
    },
    /// 파일 데이터 한 프레임(보내는 쪽 → 받는 쪽) — 요청 블록 안의 연속 구간.
    FileData {
        req: u32,
        offset: u64,
        data: Vec<u8>,
    },
    /// 블록 끝 — 실제 파일 크기·수정시각(받는 쪽이 원본 변경을 알아채는 근거).
    FileEnd {
        req: u32,
        size: u64,
        mtime: u64,
    },
    /// 요청 거절/실패.
    FileErr {
        req: u32,
        code: FileErrCode,
        msg: String,
    },
    /// 받는 쪽이 요청을 거둔다(보내는 쪽은 남은 프레임을 버린다).
    FileCancel {
        req: u32,
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
            PeerMsg::FileReq {
                req,
                offset,
                len,
                path,
            } => {
                let p = path.as_bytes();
                let p = &p[..p.len().min(FILE_PATH_MAX)];
                let mut v = Vec::with_capacity(22 + p.len());
                v.extend_from_slice(TAG_FREQ);
                v.extend_from_slice(&req.to_le_bytes());
                v.extend_from_slice(&offset.to_le_bytes());
                v.extend_from_slice(&len.to_le_bytes());
                v.extend_from_slice(&(p.len() as u16).to_le_bytes());
                v.extend_from_slice(p);
                v
            }
            PeerMsg::FileData { req, offset, data } => {
                let mut v = Vec::with_capacity(16 + data.len());
                v.extend_from_slice(TAG_FDAT);
                v.extend_from_slice(&req.to_le_bytes());
                v.extend_from_slice(&offset.to_le_bytes());
                v.extend_from_slice(data);
                v
            }
            PeerMsg::FileEnd { req, size, mtime } => {
                let mut v = Vec::with_capacity(24);
                v.extend_from_slice(TAG_FEND);
                v.extend_from_slice(&req.to_le_bytes());
                v.extend_from_slice(&size.to_le_bytes());
                v.extend_from_slice(&mtime.to_le_bytes());
                v
            }
            PeerMsg::FileErr { req, code, msg } => {
                let m = msg.as_bytes();
                let m = &m[..m.len().min(255)];
                let mut v = Vec::with_capacity(9 + m.len());
                v.extend_from_slice(TAG_FERR);
                v.extend_from_slice(&req.to_le_bytes());
                v.push(code.to_byte());
                v.extend_from_slice(m);
                v
            }
            PeerMsg::FileCancel { req } => {
                let mut v = Vec::with_capacity(8);
                v.extend_from_slice(TAG_FCAN);
                v.extend_from_slice(&req.to_le_bytes());
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
            t if t == TAG_FREQ => {
                let (req, rest) = rest.split_at_checked(4)?;
                let (off, rest) = rest.split_at_checked(8)?;
                let (len, rest) = rest.split_at_checked(4)?;
                let (pl, rest) = rest.split_at_checked(2)?;
                let pl = usize::from(u16::from_le_bytes(pl.try_into().ok()?));
                let (path, _) = rest.split_at_checked(pl)?;
                Some(PeerMsg::FileReq {
                    req: u32::from_le_bytes(req.try_into().ok()?),
                    offset: u64::from_le_bytes(off.try_into().ok()?),
                    len: u32::from_le_bytes(len.try_into().ok()?),
                    path: std::str::from_utf8(path).ok()?.to_string(),
                })
            }
            t if t == TAG_FDAT => {
                let (req, rest) = rest.split_at_checked(4)?;
                let (off, data) = rest.split_at_checked(8)?;
                Some(PeerMsg::FileData {
                    req: u32::from_le_bytes(req.try_into().ok()?),
                    offset: u64::from_le_bytes(off.try_into().ok()?),
                    data: data.to_vec(),
                })
            }
            t if t == TAG_FEND => {
                let (req, rest) = rest.split_at_checked(4)?;
                let (size, rest) = rest.split_at_checked(8)?;
                let (mtime, _) = rest.split_at_checked(8)?;
                Some(PeerMsg::FileEnd {
                    req: u32::from_le_bytes(req.try_into().ok()?),
                    size: u64::from_le_bytes(size.try_into().ok()?),
                    mtime: u64::from_le_bytes(mtime.try_into().ok()?),
                })
            }
            t if t == TAG_FERR => {
                let (req, rest) = rest.split_at_checked(4)?;
                let (code, msg) = rest.split_first()?;
                Some(PeerMsg::FileErr {
                    req: u32::from_le_bytes(req.try_into().ok()?),
                    code: FileErrCode::from_byte(*code),
                    msg: String::from_utf8_lossy(msg).into_owned(),
                })
            }
            t if t == TAG_FCAN => {
                let (req, _) = rest.split_at_checked(4)?;
                Some(PeerMsg::FileCancel {
                    req: u32::from_le_bytes(req.try_into().ok()?),
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

    /// ★ 파일 메시지 5종 왕복(09-12) — 오프셋·길이·사유 코드가 그대로 돌아온다.
    #[test]
    fn file_messages_roundtrip() {
        let msgs = [
            PeerMsg::FileReq {
                req: 7,
                offset: 1 << 33,
                len: 262_144,
                path: "D:/작업/보고서 초안.xlsx".into(),
            },
            PeerMsg::FileData {
                req: 7,
                offset: 1 << 33,
                data: vec![9u8; 1234],
            },
            PeerMsg::FileEnd {
                req: 7,
                size: 12_345_678_901,
                mtime: 1_757_000_000,
            },
            PeerMsg::FileErr {
                req: 7,
                code: FileErrCode::NotOffered,
                msg: "not offered".into(),
            },
            PeerMsg::FileCancel { req: 7 },
        ];
        for m in msgs {
            assert_eq!(PeerMsg::decode(&m.encode()), Some(m.clone()), "{m:?}");
        }
        // 미지 사유 코드는 Other로(전방 호환) · 잘린 프레임은 None.
        let mut err = b"NCF4".to_vec();
        err.extend_from_slice(&7u32.to_le_bytes());
        err.push(99);
        err.push(b'x');
        assert_eq!(
            PeerMsg::decode(&err),
            Some(PeerMsg::FileErr {
                req: 7,
                code: FileErrCode::Other,
                msg: "x".into()
            })
        );
        let mut short = b"NCF3".to_vec();
        short.extend_from_slice(&7u32.to_le_bytes());
        short.push(1);
        assert_eq!(PeerMsg::decode(&short), None);
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
