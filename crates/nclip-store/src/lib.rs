//! `nclip-store` — 영속(T-16 · [DR-37](../../../docs/10-decision-record.md)). **DB 엔진을 링크하지 않는다.**
//!
//! ```text
//! data/store/
//! ├─ device.key            기기 로컬 비밀(keys 참조)
//! ├─ keys                  래핑된 마스터 키
//! ├─ index/seg-NNNNNN.idx  append-only 이벤트 로그(레코드마다 AEAD 봉투)
//! └─ blob/xy/<hex64>       내용 주소 blob(큰 표현 · blob_id = 키 있는 평문 해시)
//! ```
//!
//! 접근 패턴에 join도 트랜잭션 경합도 없다 — **쓰기는 끝에 붙이고, 재생으로 되살리고,
//! 살아 있는 것만 새 세그먼트로 압축한다**(로그가 잘하는 문제다 · [docs/06](../../../docs/06-storage-design.md)).
//!
//! | 원칙 | 구현 |
//! |---|---|
//! | 암호화 기본(DR-38) | 레코드·blob 전부 [`sealed`] 봉투 — 평문 폴백 없음 |
//! | 중복 제거(DR-37d) | `blob_id = SHA-256("nclip/blob-id-v1" ‖ master ‖ H(평문))` — 키 있는 파생이라 **확인 공격 오라클이 없다**(D-71 소멸 논리), 같은 평문 = 같은 파일 |
//! | 틀리면 교체(DR-37f) | [`HistoryStore`] 트레이트 — 셸은 파일 형식을 모른다 |
//! | 실패해도 앱은 산다(DR-31) | 저장 실패 = 로그 + `degraded` 플래그. 이력은 메모리에 살아 있다 |
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod codec;
mod keys;
pub mod sealed;

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use nclip_core::{ClipKind, RawRep};

const DOMAIN_IDX: &[u8] = b"idx-v1";
const DOMAIN_BLOB: &[u8] = b"blob-v1";
/// 이보다 큰 표현은 인덱스가 아니라 blob으로 — 인덱스는 작아야 재생이 싸다([docs/06 §2-1]).
const BLOB_MIN: usize = 16 * 1024;
/// 죽은 이벤트가 이 배율을 넘으면 열 때 압축한다.
const COMPACT_RATIO: usize = 3;

// ─────────────────────────────────────────────── 포트

/// 저장된 항목 — 이력 항목의 영속 형상(지문은 적재 때 재계산한다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredItem {
    /// 항목 id — 셸이 부여하는 단조 증가 값(재시작 후에도 이어진다).
    pub id: u64,
    pub kind: ClipKind,
    pub label: String,
    pub reps: Vec<RawRep>,
    pub source_app: Option<String>,
    pub copies: u32,
    /// ★ 핀(T-18b0 기초) — 상한 제거에서 지켜진다. UI는 후속.
    pub pinned: bool,
    pub thumb: Option<(u32, u32, Vec<u8>)>,
}

/// 이력 영속 포트(DR-37f) — 셸은 이것만 안다.
pub trait HistoryStore {
    /// 전체 재생 — 최신이 앞. 손상 레코드는 세고 버린다(fail-closed).
    fn load(&mut self) -> Vec<StoredItem>;
    /// 새 항목(또는 같은 id의 교체 — coalesce 완본).
    fn add(&mut self, item: &StoredItem);
    /// 승격 — 맨 앞으로 + `copies` +1.
    fn touch(&mut self, id: u64);
    /// 제거(상한 축출 포함).
    fn remove(&mut self, id: u64);
    /// ★ 전부 지운다(`sec.clear_on_quit`) — 세그먼트·blob 실파일 삭제.
    fn wipe(&mut self);
    /// 저장이 한 번이라도 실패했는가(상태 표시용 — DR-31).
    fn degraded(&self) -> bool;
}

/// 저장 없이 도는 자리(열기 실패·테스트) — 전부 no-op.
#[derive(Debug)]
pub struct NullStore;

impl HistoryStore for NullStore {
    fn load(&mut self) -> Vec<StoredItem> {
        Vec::new()
    }
    fn add(&mut self, _: &StoredItem) {}
    fn touch(&mut self, _: u64) {}
    fn remove(&mut self, _: u64) {}
    fn wipe(&mut self) {}
    fn degraded(&self) -> bool {
        false
    }
}

// ─────────────────────────────────────────────── 파일 구현

/// [`FileStore::open`] 결과 — 셸이 콘솔·상태에 알릴 사실.
#[derive(Debug)]
pub struct OpenReport {
    pub store: FileStore,
    /// ★ 기기 키 불일치로 기존 기록을 `.locked`로 보관하고 새로 시작했다.
    pub archived: bool,
}

pub struct FileStore {
    dir: PathBuf,
    master: [u8; 32],
    /// 현재 세그먼트 번호(파일명 `seg-{n:06}.idx`).
    seg_no: u32,
    /// 재생으로 센 이벤트 수(압축 판단).
    events: usize,
    /// blob 참조 수 — 0이 되면 실파일을 지운다.
    blob_refs: HashMap<[u8; 32], u32>,
    /// 항목 → 참조 blob 목록(제거 때 감산용).
    item_blobs: HashMap<u64, Vec<[u8; 32]>>,
    degraded: bool,
}

// ★ 수동 Debug — 마스터 키는 어떤 로그에도 안 나간다.
impl std::fmt::Debug for FileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStore")
            .field("dir", &self.dir)
            .field("seg_no", &self.seg_no)
            .field("events", &self.events)
            .field("degraded", &self.degraded)
            .finish_non_exhaustive()
    }
}

impl FileStore {
    /// `dir`(예: `data/store`)에서 연다. 폴더·키가 없으면 만든다.
    ///
    /// # Errors
    /// 폴더 생성·키 생성 실패(디스크가 아예 안 되는 자리) — 호출측은 [`NullStore`]로 강등한다.
    pub fn open(dir: &Path) -> std::io::Result<OpenReport> {
        std::fs::create_dir_all(dir.join("index"))?;
        std::fs::create_dir_all(dir.join("blob"))?;
        let (master, archived) = match keys::load_master(dir)? {
            keys::MasterLoad::Ready(m) => (m, false),
            keys::MasterLoad::Mismatch => {
                // ★ 보관은 삭제가 아니다 — index·blob·keys를 .locked로 옮기고 새로 시작.
                for name in ["index", "blob", "keys"] {
                    let p = dir.join(name);
                    if p.exists() {
                        if let Some(arch) = keys::archive_name(&p) {
                            let _ = std::fs::rename(&p, &arch);
                        }
                    }
                }
                std::fs::create_dir_all(dir.join("index"))?;
                std::fs::create_dir_all(dir.join("blob"))?;
                let keys::MasterLoad::Ready(m) = keys::load_master(dir)? else {
                    return Err(std::io::Error::other("키 재생성 실패"));
                };
                (m, true)
            }
        };
        let seg_no = last_seg_no(&dir.join("index")).unwrap_or(0).max(1);
        Ok(OpenReport {
            store: Self {
                dir: dir.to_path_buf(),
                master,
                seg_no,
                events: 0,
                blob_refs: HashMap::new(),
                item_blobs: HashMap::new(),
                degraded: false,
            },
            archived,
        })
    }

    fn seg_path(&self, n: u32) -> PathBuf {
        self.dir.join("index").join(format!("seg-{n:06}.idx"))
    }

    fn blob_path(&self, id: &[u8; 32]) -> PathBuf {
        let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join("blob").join(&hex[..2]).join(hex)
    }

    /// ★ 키 있는 내용 주소(DR-37d) — 평문 해시를 마스터로 감싼다(확인 공격 오라클 차단).
    fn blob_id(&self, plain: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let inner: [u8; 32] = Sha256::digest(plain).into();
        let mut h = Sha256::new();
        h.update(b"nclip/blob-id-v1");
        h.update(self.master);
        h.update(inner);
        h.finalize().into()
    }

    fn fail(&mut self, what: &str, e: &dyn std::fmt::Display) {
        // 같은 오류를 도배하지 않는다 — 첫 실패만 찍고 플래그를 세운다(DR-31).
        if !self.degraded {
            eprintln!("저장 실패({what}): {e} — 이력은 이 세션 동안 메모리에 유지됩니다");
        }
        self.degraded = true;
    }

    fn append_record(&mut self, plain: &[u8]) {
        let rec = match sealed::seal(DOMAIN_IDX, &self.master, plain) {
            Ok(s) => s,
            Err(e) => return self.fail("봉인", &e),
        };
        let path = self.seg_path(self.seg_no);
        let r = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| {
                f.write_all(&u32::try_from(rec.len()).unwrap_or(0).to_le_bytes())?;
                f.write_all(&rec)
            });
        match r {
            Ok(()) => self.events += 1,
            Err(e) => self.fail("쓰기", &e),
        }
    }

    fn write_blob(&mut self, plain: &[u8]) -> Option<[u8; 32]> {
        let id = self.blob_id(plain);
        let path = self.blob_path(&id);
        if !path.exists() {
            let parent = path.parent()?;
            let rec = match sealed::seal(DOMAIN_BLOB, &self.master, plain) {
                Ok(s) => s,
                Err(e) => {
                    self.fail("blob 봉인", &e);
                    return None;
                }
            };
            if let Err(e) =
                std::fs::create_dir_all(parent).and_then(|()| std::fs::write(&path, &rec))
            {
                self.fail("blob 쓰기", &e);
                return None;
            }
        }
        Some(id)
    }

    fn read_blob(&self, id: &[u8; 32]) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.blob_path(id)).ok()?;
        sealed::open(DOMAIN_BLOB, &self.master, &bytes)
    }

    /// Add 레코드 본문 인코딩 — blob 참조 수·항목→blob 표를 갱신한다.
    fn encode_add(&mut self, item: &StoredItem) -> Vec<u8> {
        let mut w = codec::W::new();
        w.u8(EV_ADD);
        w.u64(item.id);
        w.u8(kind_code(item.kind));
        w.u32(item.copies);
        w.u8(u8::from(item.pinned));
        w.opt_str(item.source_app.as_deref());
        w.str(&item.label);
        match &item.thumb {
            Some((tw, th, rgba)) => {
                w.u8(1);
                w.u32(*tw);
                w.u32(*th);
                w.bytes(rgba);
            }
            None => w.u8(0),
        }
        w.u32(u32::try_from(item.reps.len()).unwrap_or(0));
        let mut ids = Vec::new();
        for r in &item.reps {
            w.str(&r.format);
            if r.data.len() >= BLOB_MIN {
                if let Some(id) = self.write_blob(&r.data) {
                    w.u8(1);
                    w.0.extend_from_slice(&id);
                    w.u64(r.data.len() as u64);
                    ids.push(id);
                    continue;
                }
                // blob 실패 — 인라인 폴백(레코드가 커지지만 데이터는 산다).
            }
            w.u8(0);
            w.bytes(&r.data);
        }
        for id in &ids {
            *self.blob_refs.entry(*id).or_insert(0) += 1;
        }
        self.item_blobs.insert(item.id, ids);
        w.0
    }

    fn decode_add(&self, body: &[u8]) -> Option<(StoredItem, Vec<[u8; 32]>)> {
        let mut r = codec::R(body);
        let id = r.u64()?;
        let kind = kind_from(r.u8()?)?;
        let copies = r.u32()?;
        let pinned = r.u8()? == 1;
        let source_app = r.opt_str()?;
        let label = r.str()?;
        let thumb = match r.u8()? {
            1 => Some((r.u32()?, r.u32()?, r.bytes()?.to_vec())),
            _ => None,
        };
        let n = r.u32()? as usize;
        let mut reps = Vec::with_capacity(n.min(1024));
        let mut ids = Vec::new();
        for _ in 0..n {
            let format = r.str()?;
            let data = match r.u8()? {
                1 => {
                    let (idb, rest) = r.0.split_at_checked(32)?;
                    r.0 = rest;
                    let mut bid = [0u8; 32];
                    bid.copy_from_slice(idb);
                    let len = usize::try_from(r.u64()?).ok()?;
                    let plain = self.read_blob(&bid)?;
                    if plain.len() != len {
                        return None; // blob 손상 — 항목째 버린다(반쪽 항목은 거짓말이다)
                    }
                    ids.push(bid);
                    plain
                }
                _ => r.bytes()?.to_vec(),
            };
            reps.push(RawRep { format, data });
        }
        Some((
            StoredItem {
                id,
                kind,
                label,
                reps,
                source_app,
                copies,
                pinned,
                thumb,
            },
            ids,
        ))
    }

    /// 세그먼트 전체 재생 — (살아 있는 항목: 최신 앞) + 이벤트 수.
    fn replay(&mut self) -> (Vec<StoredItem>, usize) {
        let mut items: Vec<StoredItem> = Vec::new();
        let mut events = 0usize;
        let mut dropped = 0usize;
        let dir = self.dir.join("index");
        let mut segs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "idx"))
                    .collect()
            })
            .unwrap_or_default();
        segs.sort();
        for seg in segs {
            let Ok(bytes) = std::fs::read(&seg) else {
                continue;
            };
            let mut off = 0usize;
            while off + 4 <= bytes.len() {
                let len =
                    u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap_or([0; 4])) as usize;
                let Some(rec) = bytes.get(off + 4..off + 4 + len) else {
                    break; // 잘린 꼬리(크래시 관용) — 여기까지가 진실이다
                };
                off += 4 + len;
                let Some(plain) = sealed::open(DOMAIN_IDX, &self.master, rec) else {
                    // 봉투가 안 열린다 = 손상·바꿔치기 — 이 세그먼트의 나머지는 못 믿는다.
                    dropped += 1;
                    break;
                };
                events += 1;
                let mut r = codec::R(&plain);
                match r.u8() {
                    Some(EV_ADD) => {
                        if let Some((item, ids)) = self.decode_add(&plain[1..]) {
                            for id in &ids {
                                *self.blob_refs.entry(*id).or_insert(0) += 1;
                            }
                            if let Some(old) = items.iter().position(|it| it.id == item.id) {
                                let old_id = items[old].id;
                                items.remove(old);
                                self.deref_blobs_of(old_id);
                            }
                            self.item_blobs.insert(item.id, ids);
                            items.insert(0, item);
                        } else {
                            dropped += 1;
                        }
                    }
                    Some(EV_TOUCH) => {
                        if let Some(id) = r.u64() {
                            if let Some(i) = items.iter().position(|it| it.id == id) {
                                let mut it = items.remove(i);
                                it.copies += 1;
                                items.insert(0, it);
                            }
                        }
                    }
                    Some(EV_REMOVE) => {
                        if let Some(id) = r.u64() {
                            if let Some(i) = items.iter().position(|it| it.id == id) {
                                items.remove(i);
                                self.deref_blobs_of(id);
                            }
                        }
                    }
                    _ => dropped += 1,
                }
            }
        }
        if dropped > 0 {
            eprintln!("저장소: 손상 레코드 {dropped}건 건너뜀(fail-closed)");
        }
        (items, events)
    }

    fn deref_blobs_of(&mut self, item_id: u64) {
        for id in self.item_blobs.remove(&item_id).unwrap_or_default() {
            if let Some(n) = self.blob_refs.get_mut(&id) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    self.blob_refs.remove(&id);
                    let _ = std::fs::remove_file(self.blob_path(&id));
                }
            }
        }
    }

    /// 살아 있는 항목만 새 세그먼트로 다시 쓰고 옛 세그먼트를 지운다.
    fn compact(&mut self, items: &[StoredItem]) {
        let old: Vec<PathBuf> = (0..=self.seg_no).map(|n| self.seg_path(n)).collect();
        self.seg_no += 1;
        self.events = 0;
        // 최신이 앞인 목록을 **오래된 것부터** Add — 재생하면 같은 순서가 된다.
        for it in items.iter().rev() {
            let plain = self.encode_add(it);
            self.append_record(&plain);
            // 참조 수는 재생 때 이미 셌다 — encode_add의 재가산을 상쇄한다.
            for id in self.item_blobs.get(&it.id).cloned().unwrap_or_default() {
                if let Some(n) = self.blob_refs.get_mut(&id) {
                    *n = n.saturating_sub(1);
                }
            }
        }
        for p in old {
            let _ = std::fs::remove_file(p);
        }
    }
}

const EV_ADD: u8 = 1;
const EV_TOUCH: u8 = 2;
const EV_REMOVE: u8 = 3;

fn kind_code(k: ClipKind) -> u8 {
    match k {
        ClipKind::Text => 0,
        ClipKind::RichText => 1,
        ClipKind::Image => 2,
        ClipKind::Object => 3,
        ClipKind::Files => 4,
        ClipKind::Color => 5,
    }
}

fn kind_from(c: u8) -> Option<ClipKind> {
    Some(match c {
        0 => ClipKind::Text,
        1 => ClipKind::RichText,
        2 => ClipKind::Image,
        3 => ClipKind::Object,
        4 => ClipKind::Files,
        5 => ClipKind::Color,
        _ => return None,
    })
}

fn last_seg_no(index_dir: &Path) -> Option<u32> {
    std::fs::read_dir(index_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.path()
                .file_stem()?
                .to_str()?
                .strip_prefix("seg-")?
                .parse::<u32>()
                .ok()
        })
        .max()
}

impl HistoryStore for FileStore {
    fn load(&mut self) -> Vec<StoredItem> {
        let (items, events) = self.replay();
        self.events = events;
        // 죽은 이벤트가 살아 있는 항목의 몇 배면 열 때 한 번 압축한다.
        if events > items.len() * COMPACT_RATIO + 64 {
            self.compact(&items);
        }
        items
    }

    fn add(&mut self, item: &StoredItem) {
        let plain = self.encode_add(item);
        self.append_record(&plain);
    }

    fn touch(&mut self, id: u64) {
        let mut w = codec::W::new();
        w.u8(EV_TOUCH);
        w.u64(id);
        self.append_record(&w.0);
    }

    fn remove(&mut self, id: u64) {
        let mut w = codec::W::new();
        w.u8(EV_REMOVE);
        w.u64(id);
        self.append_record(&w.0);
        self.deref_blobs_of(id);
    }

    fn wipe(&mut self) {
        let _ = std::fs::remove_dir_all(self.dir.join("index"));
        let _ = std::fs::remove_dir_all(self.dir.join("blob"));
        let _ = std::fs::create_dir_all(self.dir.join("index"));
        let _ = std::fs::create_dir_all(self.dir.join("blob"));
        self.blob_refs.clear();
        self.item_blobs.clear();
        self.events = 0;
        self.seg_no += 1;
    }

    fn degraded(&self) -> bool {
        self.degraded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nclip-store-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn item(id: u64, label: &str, data: &[u8]) -> StoredItem {
        StoredItem {
            id,
            kind: ClipKind::Text,
            label: label.into(),
            reps: vec![RawRep {
                format: "CF_UNICODETEXT".into(),
                data: data.to_vec(),
            }],
            source_app: Some("test".into()),
            copies: 1,
            pinned: false,
            thumb: None,
        }
    }

    /// ★ 넣은 것이 다시 열면 그대로 온다 — 이 크레이트의 존재 이유.
    #[test]
    fn add_survives_reopen_newest_first() {
        let d = tmp("roundtrip");
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "하나", b"one"));
            s.add(&item(2, "둘", b"two"));
        }
        let mut s = FileStore::open(&d).unwrap().store;
        let items = s.load();
        assert_eq!(
            items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![2, 1],
            "최신이 앞"
        );
        assert_eq!(items[0].label, "둘");
        assert_eq!(items[1].reps[0].data, b"one");
        assert!(!s.degraded());
        let _ = std::fs::remove_dir_all(d);
    }

    /// 승격·제거가 재생에 반영된다.
    #[test]
    fn touch_reorders_and_remove_deletes() {
        let d = tmp("events");
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "a", b"a"));
            s.add(&item(2, "b", b"b"));
            s.add(&item(3, "c", b"c"));
            s.touch(1); // a를 맨 위로
            s.remove(2); // b 삭제
        }
        let mut s = FileStore::open(&d).unwrap().store;
        let items = s.load();
        assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(items[0].copies, 2, "승격 = copies +1");
        let _ = std::fs::remove_dir_all(d);
    }

    /// ★ 큰 표현은 blob으로 가고, 같은 평문은 **한 파일**이다(중복 제거 — DR-37d).
    #[test]
    fn big_reps_dedup_into_one_blob() {
        let d = tmp("blob");
        let big = vec![7u8; BLOB_MIN + 1];
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "x", &big));
            s.add(&item(2, "y", &big));
        }
        let count = walk_files(&d.join("blob"));
        assert_eq!(count, 1, "같은 평문 = 같은 blob_id = 한 파일");
        let mut s = FileStore::open(&d).unwrap().store;
        let items = s.load();
        assert_eq!(items[0].reps[0].data, big, "blob이 되살아난다");
        // 한 항목을 지워도 파일은 남고(공유), 둘 다 지우면 사라진다.
        s.remove(1);
        assert_eq!(walk_files(&d.join("blob")), 1);
        s.remove(2);
        assert_eq!(walk_files(&d.join("blob")), 0, "참조 0 = 실파일 삭제");
        let _ = std::fs::remove_dir_all(d);
    }

    /// 디스크의 blob·레코드는 평문이 아니다(DR-38 — 암호화 기본).
    #[test]
    fn nothing_on_disk_is_plaintext() {
        let d = tmp("sealed");
        let big = vec![0x41u8; BLOB_MIN + 100]; // "AAAA…"
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "비밀텍스트", &big));
        }
        for f in all_files(&d) {
            let bytes = std::fs::read(&f).unwrap();
            let looks_plain = bytes
                .windows(8)
                .any(|w| w == b"AAAAAAAA" || w == "비밀".as_bytes());
            assert!(!looks_plain, "{} 에 평문이 보인다", f.display());
        }
        let _ = std::fs::remove_dir_all(d);
    }

    /// 잘린 꼬리(크래시)는 그 앞까지 살린다 — 전체를 버리지 않는다.
    #[test]
    fn truncated_tail_keeps_earlier_records() {
        let d = tmp("trunc");
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "a", b"a"));
            s.add(&item(2, "b", b"b"));
        }
        // 세그먼트 꼬리를 몇 바이트 자른다(마지막 레코드 파괴).
        let seg = d.join("index").join("seg-000001.idx");
        let bytes = std::fs::read(&seg).unwrap();
        std::fs::write(&seg, &bytes[..bytes.len() - 3]).unwrap();
        let mut s = FileStore::open(&d).unwrap().store;
        let items = s.load();
        assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1]);
        let _ = std::fs::remove_dir_all(d);
    }

    /// ★ 기기 키가 바뀌면 기존 기록은 `.locked` 보관 + 빈 상태로 새 출발(fail-closed).
    #[test]
    fn device_key_change_archives_and_starts_fresh() {
        let d = tmp("lock");
        {
            let mut s = FileStore::open(&d).unwrap().store;
            s.add(&item(1, "a", b"a"));
        }
        std::fs::write(d.join("device.key"), [9u8; 32]).unwrap();
        let rep = FileStore::open(&d).unwrap();
        assert!(rep.archived, "보관 사실을 알린다");
        let mut s = rep.store;
        assert!(s.load().is_empty(), "새 출발");
        assert!(d.join("index.locked").exists(), "보관은 삭제가 아니다");
        let _ = std::fs::remove_dir_all(d);
    }

    /// wipe는 실파일까지 지운다(`sec.clear_on_quit`).
    #[test]
    fn wipe_deletes_files() {
        let d = tmp("wipe");
        let mut s = FileStore::open(&d).unwrap().store;
        s.add(&item(1, "a", &vec![1u8; BLOB_MIN + 1]));
        s.wipe();
        assert_eq!(walk_files(&d.join("blob")), 0);
        assert_eq!(walk_files(&d.join("index")), 0);
        let mut s2 = FileStore::open(&d).unwrap().store;
        assert!(s2.load().is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    /// 죽은 이벤트가 쌓이면 열 때 압축된다 — 로그가 무한히 크지 않는다(DR-9).
    #[test]
    fn compaction_shrinks_log() {
        let d = tmp("compact");
        {
            let mut s = FileStore::open(&d).unwrap().store;
            for i in 0..80 {
                s.add(&item(i, "x", b"x"));
                s.remove(i);
            }
            s.add(&item(999, "산다", b"live"));
        }
        let mut s = FileStore::open(&d).unwrap().store;
        let items = s.load();
        assert_eq!(items.len(), 1);
        // 압축 후 다시 열면 이벤트는 살아 있는 항목 수뿐이다.
        let mut s2 = FileStore::open(&d).unwrap().store;
        let again = s2.load();
        assert_eq!(again.len(), 1);
        assert_eq!(s2.events, 1, "Add 1건으로 압축됐다");
        let _ = std::fs::remove_dir_all(d);
    }

    fn all_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                out.extend(all_files(&p));
            } else {
                out.push(p);
            }
        }
        out
    }

    fn walk_files(dir: &Path) -> usize {
        all_files(dir).len()
    }
}
