//! ★ **원격 파일 매니페스트**(09-12 · DR-30 · [docs/26](../../../docs/26-file-content-sharing.md)) —
//! 다른 기기에서 복사한 파일 항목의 **약속**(promise). 내용은 여기 없다: 경로·크기·수정시각과
//! **누구에게 달라고 할지**(원본 `PeerId`)만 들고, 붙여넣을 때 그 기기에서 당겨 받는다.
//!
//! 표현 이름 [`FORMAT`]으로 이력 항목의 `reps`에 **한 표현으로** 들어간다 — 그래서 영속(저장소는
//! 표현을 이름째 저장한다)·중복 제거·삭제가 공짜로 따라온다. OS 클립보드에는 **올리지 않는다**
//! (셸이 게시 직전에 걸러낸다 — 캐시된 실파일의 `CF_HDROP` 등으로 바꿔 올린다).
//!
//! ```text
//! "NCRF" ‖ ver u8(1) ‖ hex_len u8 ‖ origin_hex ‖ name_len u8 ‖ origin_name
//!        ‖ n u16 ‖ [path_len u16 ‖ path ‖ size u64 ‖ mtime u64]*
//! ```
//! 전파 페이로드의 `x-nclip/files` 파트도 **같은 인코딩**을 쓴다(origin은 비워 보내고 받는 쪽이
//! 세션의 PeerId로 채운다 — 보내는 쪽이 자기 신원을 적어 봐야 받는 쪽은 세션을 믿지 그 글자를 믿지 않는다).

/// 표현 이름 — [`crate::capture::is_files_format`]이 파일 항목으로 판정한다.
pub const FORMAT: &str = "x-nclip/remote-files";
const MAGIC: &[u8; 4] = b"NCRF";

/// 원격 파일 하나 — 원본 기기의 경로와 복사 시점의 크기·수정시각(초).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFile {
    /// 원본 기기의 경로(그 OS 표기 그대로).
    pub path: String,
    /// 바이트 크기(복사 시점) — 상한 판정·진행률 분모.
    pub size: u64,
    /// 수정 시각(unix 초) — 받는 도중 원본이 바뀌었는지 알아채는 근거.
    pub mtime: u64,
}

impl RemoteFile {
    /// 표시 이름(경로의 마지막 조각 · NFD 조합).
    #[must_use]
    pub fn name(&self) -> String {
        crate::capture::compose_hangul_nfd(crate::capture::base_name(&self.path))
    }
}

/// 매니페스트 — 원본 기기 + 파일 목록.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RemoteFiles {
    /// 원본 기기 PeerId 16진(세션이 인증한 값 — 받는 쪽이 채운다).
    pub origin_hex: String,
    /// 원본 기기 표시 이름(목록·로그용).
    pub origin_name: String,
    /// 파일들(복사 순서).
    pub files: Vec<RemoteFile>,
}

impl RemoteFiles {
    /// 합계 바이트 — 상한은 **합계**로 본다(docs/26 D-68).
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// 경로 목록(순서 유지).
    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }

    /// 직렬화.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let hex = self.origin_hex.as_bytes();
        let hex = &hex[..hex.len().min(255)];
        let name = self.origin_name.as_bytes();
        let name = &name[..name.len().min(255)];
        let mut v = Vec::with_capacity(
            9 + hex.len()
                + name.len()
                + self.files.iter().map(|f| f.path.len() + 18).sum::<usize>(),
        );
        v.extend_from_slice(MAGIC);
        v.push(1);
        v.push(hex.len() as u8);
        v.extend_from_slice(hex);
        v.push(name.len() as u8);
        v.extend_from_slice(name);
        let n = self.files.len().min(usize::from(u16::MAX));
        v.extend_from_slice(&(n as u16).to_le_bytes());
        for f in self.files.iter().take(n) {
            let p = f.path.as_bytes();
            let p = &p[..p.len().min(usize::from(u16::MAX))];
            v.extend_from_slice(&(p.len() as u16).to_le_bytes());
            v.extend_from_slice(p);
            v.extend_from_slice(&f.size.to_le_bytes());
            v.extend_from_slice(&f.mtime.to_le_bytes());
        }
        v
    }

    /// 역직렬화 — 형식 위반·미지 버전은 `None`.
    #[must_use]
    pub fn decode(b: &[u8]) -> Option<Self> {
        let rest = b.strip_prefix(MAGIC)?;
        let (ver, rest) = rest.split_first()?;
        if *ver != 1 {
            return None;
        }
        let (hl, rest) = rest.split_first()?;
        let (hex, rest) = rest.split_at_checked(usize::from(*hl))?;
        let (nl, rest) = rest.split_first()?;
        let (name, rest) = rest.split_at_checked(usize::from(*nl))?;
        let (n, mut rest) = rest.split_at_checked(2)?;
        let n = usize::from(u16::from_le_bytes(n.try_into().ok()?));
        let mut files = Vec::with_capacity(n);
        for _ in 0..n {
            let (pl, r) = rest.split_at_checked(2)?;
            let pl = usize::from(u16::from_le_bytes(pl.try_into().ok()?));
            let (path, r) = r.split_at_checked(pl)?;
            let (size, r) = r.split_at_checked(8)?;
            let (mtime, r) = r.split_at_checked(8)?;
            files.push(RemoteFile {
                path: std::str::from_utf8(path).ok()?.to_string(),
                size: u64::from_le_bytes(size.try_into().ok()?),
                mtime: u64::from_le_bytes(mtime.try_into().ok()?),
            });
            rest = r;
        }
        Some(Self {
            origin_hex: std::str::from_utf8(hex).ok()?.to_string(),
            origin_name: std::str::from_utf8(name).ok()?.to_string(),
            files,
        })
    }

    /// 표현 묶음에서 매니페스트를 찾는다(없으면 `None` = 로컬 파일 항목이거나 파일 아님).
    #[must_use]
    pub fn of_reps(reps: &[crate::RawRep]) -> Option<Self> {
        reps.iter()
            .find(|r| r.format == FORMAT)
            .and_then(|r| Self::decode(&r.data))
    }
}

/// ★ 캐시 열쇠 — `(원본 기기, 경로, 크기, 수정시각)`이 같으면 **같은 파일**이다(32자 16진).
///
/// 내용 해시가 아니다: 복사 시점에 원본 기기가 파일 전체를 읽어 해시하면 GB급에서 복사 자체가
/// 느려진다(docs/26 P-2 "스트리밍"과 상충). 메타 4조가 같은데 내용이 다른 경우는
/// 같은 초 안에 같은 크기로 덮어쓴 파일뿐이라 캐시 열쇠로는 충분하다.
/// 외부 crate 없이(DR-8) FNV-1a 64비트 두 벌(시드 다름)로 128비트를 만든다.
#[must_use]
pub fn cache_key(origin_hex: &str, f: &RemoteFile) -> String {
    fn fnv(seed: u64, parts: &[&[u8]]) -> u64 {
        let mut h = seed;
        for p in parts {
            for b in *p {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h ^= 0xff;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
    let parts: [&[u8]; 4] = [
        origin_hex.as_bytes(),
        f.path.as_bytes(),
        &f.size.to_le_bytes(),
        &f.mtime.to_le_bytes(),
    ];
    let a = fnv(0xcbf2_9ce4_8422_2325, &parts);
    let b = fnv(0x84222325_cbf29ce4, &parts);
    format!("{a:016x}{b:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RemoteFiles {
        RemoteFiles {
            origin_hex: "ab".repeat(32),
            origin_name: "A-데스크톱".into(),
            files: vec![
                RemoteFile {
                    path: "D:\\작업\\보고서 초안.xlsx".into(),
                    size: 3_200_000_000,
                    mtime: 1_757_600_000,
                },
                RemoteFile {
                    path: "D:\\작업\\초안.md".into(),
                    size: 1234,
                    mtime: 1_757_600_001,
                },
            ],
        }
    }

    #[test]
    fn roundtrip_and_totals() {
        let m = sample();
        let d = RemoteFiles::decode(&m.encode()).expect("decode");
        assert_eq!(d, m);
        assert_eq!(d.total_bytes(), 3_200_001_234);
        assert_eq!(d.files[0].name(), "보고서 초안.xlsx");
        assert!(RemoteFiles::decode(b"NCRF\x02").is_none(), "미지 버전");
        assert!(RemoteFiles::decode(&m.encode()[..20]).is_none(), "잘림");
    }

    #[test]
    fn found_in_reps_by_format_name() {
        let m = sample();
        let reps = vec![
            crate::RawRep {
                format: "CF_UNICODETEXT".into(),
                data: vec![0; 4],
            },
            crate::RawRep {
                format: FORMAT.into(),
                data: m.encode(),
            },
        ];
        assert_eq!(RemoteFiles::of_reps(&reps), Some(m));
        assert!(RemoteFiles::of_reps(&reps[..1]).is_none());
    }

    /// 열쇠는 메타 4조에만 의존한다 — 이름이 같아도 크기가 다르면 다른 파일.
    #[test]
    fn cache_key_depends_on_all_four() {
        let m = sample();
        let k0 = cache_key(&m.origin_hex, &m.files[0]);
        assert_eq!(k0.len(), 32);
        assert_eq!(k0, cache_key(&m.origin_hex, &m.files[0]), "결정적");
        let mut other = m.files[0].clone();
        other.size += 1;
        assert_ne!(k0, cache_key(&m.origin_hex, &other));
        assert_ne!(k0, cache_key("cd", &m.files[0]));
    }
}
