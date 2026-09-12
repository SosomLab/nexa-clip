//! ★ **파일 내용 전송 관리자**(09-12 · DR-30 · [docs/26](../../../docs/26-file-content-sharing.md)) —
//! 다른 기기의 파일 항목을 **붙여넣을 때** 원본 기기에서 당겨 받고, 받은 파일을 **로컬 캐시**에
//! 남겨 다시 쓴다. 이어 받기·저속 백그라운드 사전 캐시·우선순위·진행률·중지가 전부 여기 있다.
//!
//! ## 한 줄 모델 — "다음 블록을 언제 요청하느냐"가 전부다
//!
//! 받는 쪽이 `FileReq{offset,len}`로 블록을 **당긴다**(pull). 보내는 쪽은 그 블록만 보내고 `FileEnd`로
//! 닫는다. 그래서
//! - **흐름 제어** = 앞 블록의 End를 받기 전엔 다음을 청하지 않는다(보내는 쪽 메모리 폭주 없음),
//! - **이어 받기** = `.part` 파일 길이가 곧 다음 `offset`이다(재시작·재접속 뒤에도 그대로),
//! - **저속 캐싱** = 백그라운드는 블록 사이에 `len / 속도상한`만큼 쉰다(붙여넣기 대기 전송은 쉬지 않는다),
//! - **우선순위** = 붙여넣기(Fg)가 하나라도 진행 중이면 백그라운드(Bg)는 다음 블록을 청하지 않는다.
//!
//! ## 스레드
//!
//! | 누가 | 무엇을 |
//! |---|---|
//! | UI 스레드 | [`fetch`]·[`stop`]·[`views`]·[`item_status`] — 맵 조회·짧은 stat뿐(DR-41) |
//! | 세션 스레드 | [`on_data`]/[`on_end`]/[`on_err`] — `.part`에 순차 append · [`serve`] — 블록 읽기 |
//! | 펌프 스레드([`spawn_pump`]) | 100ms마다 [`Manager::tick`] — 보낼 `FileReq` 결정 · 타임아웃 · 오프라인 전이 · UI 깨우기 |
//!
//! ## 캐시
//!
//! `<data>/cache/files/<key>/<이름>` — `key` = [`nclip_core::remote_files::cache_key`](원본 기기·경로·크기·수정시각).
//! 받는 중엔 `<이름>.part`. 완료 = 이름 바꾸기(원자적). 용량 상한을 넘으면 **오래된 디렉터리부터** 지운다
//! (전송 중인 것은 제외). 실행 파일도 그대로 둔다 — 같은 사용자의 승인된 기기에서 온 것이라
//! `nbeep-safe` 격리는 두지 않는다(docs/26 §5 — 남에게 열 때 선행 조건).

use nclip_core::remote_files::{cache_key, RemoteFile, RemoteFiles};
use nclip_core::PasteAs;
use nclip_sync::hello::{FileErrCode, PeerMsg, FILE_BLOCK_MAX, FILE_FRAME};
use std::collections::{HashMap, HashSet};
use std::io::{Read as _, Seek as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 붙여넣기 대기(Fg) 블록 — 세션 폴링 지연을 나누어 처리량을 낸다(2MiB ÷ 20ms 폴링 ≈ 100MB/s 상한).
const FG_BLOCK: u32 = 2 * 1024 * 1024;
/// 백그라운드 블록 — 작게 끊어 속도 상한을 촘촘히 지킨다.
const BG_BLOCK: u32 = 256 * 1024;
/// 블록 응답 대기 상한 — 넘으면 같은 오프셋을 다시 청한다(최대 [`MAX_RETRY`]).
const BLOCK_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RETRY: u32 = 3;
/// 동시에 블록을 청해 둘 전송 수 — Fg 2 · Bg 1(Fg가 없을 때만).
const FG_INFLIGHT: usize = 2;
/// 붙여넣기 대기 전송이 이 안에 끝나면 **자동 주입**(그보다 늦으면 클립보드 게시 + 알림만 —
/// 사용자는 이미 다른 데 가 있을 수 있다).
pub(crate) const PASTE_INJECT_WINDOW: Duration = Duration::from_secs(3);
/// 완료·실패 행을 패널에 남겨 두는 시간(그 뒤 자동으로 접힌다).
pub(crate) const LINGER: Duration = Duration::from_secs(30);

/// 우선순위.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prio {
    /// 붙여넣기가 기다린다 — 제한 없음.
    Fg,
    /// 사전 캐시 — 저속 · Fg에 양보.
    Bg,
}

/// 전송 상태.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// 차례를 기다린다(다음 블록을 청할 수 있다).
    Queued,
    /// 블록을 청해 두었다(데이터 도착 중).
    Active,
    /// 원본 기기 세션이 없다 — 돌아오면 자동 재개.
    Offline,
    Done,
    Failed(String),
    /// 사용자가 중지 — 재시도 전까지 그대로.
    Stopped,
}

/// 전송 하나(파일 단위).
#[derive(Debug)]
struct Transfer {
    req: u32,
    item_id: u64,
    origin_hex: String,
    origin_name: String,
    file: RemoteFile,
    key: String,
    prio: Prio,
    state: State,
    received: u64,
    part: Option<std::fs::File>,
    part_path: PathBuf,
    final_path: PathBuf,
    /// 다음 블록을 청해도 되는 시각(Bg 속도 상한).
    next_at: Instant,
    /// 마지막 데이터/응답 시각(타임아웃).
    last_io: Instant,
    retries: u32,
    /// 종결 시각(패널 잔류 계산).
    ended: Option<Instant>,
    /// 속도(B/s · 지수 이동 평균).
    rate: f64,
    rate_mark: (Instant, u64),
}

/// UI 스냅숏 — 패널 한 행.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct XferView {
    pub req: u32,
    pub item_id: u64,
    pub name: String,
    pub origin_name: String,
    pub size: u64,
    pub received: u64,
    pub state: State,
    pub prio: Prio,
    pub rate: f64,
}

impl XferView {
    /// 진행률(0~100).
    #[must_use]
    pub(crate) fn pct(&self) -> u8 {
        if self.size == 0 {
            return if self.state == State::Done { 100 } else { 0 };
        }
        (self.received.saturating_mul(100) / self.size).min(100) as u8
    }
}

/// 항목 단위 상태(목록 배지).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemStatus {
    /// 전부 캐시됨 — 즉시 붙여넣기.
    Cached,
    /// 받는 중(합계 진행률).
    Fetching(u8),
    /// 원본 오프라인(캐시 안 됨).
    Offline,
    Failed,
    /// 원격(아직 안 받음 — 붙여넣을 때 받는다).
    Remote,
}

/// 설정 스냅숏(바이트 단위).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    /// `sync.file_contents`.
    pub contents: bool,
    /// `sync.file_auto_mb` — 이 합계 이하면 백그라운드 사전 캐시(0 = 끔).
    pub auto_bytes: u64,
    /// `sync.file_bg_kbps` — 백그라운드 속도 상한(B/s · 0 = 무제한).
    pub bg_bps: u64,
    /// `sync.file_max_mb` — 붙여넣기 시 받아올 합계 상한(넘으면 경로만 · 0 = 무제한).
    pub max_bytes: u64,
    /// `sync.file_cache_mb` — 캐시 용량.
    pub cache_bytes: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            contents: true,
            auto_bytes: 50 * 1024 * 1024,
            bg_bps: 1024 * 1024,
            max_bytes: 1000 * 1024 * 1024,
            cache_bytes: 2000 * 1024 * 1024,
        }
    }
}

/// [`fetch`]의 결과 — 호출자(붙여넣기)가 지금 무엇을 올릴지 정한다.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Fetch {
    /// 전부 캐시됨 — 로컬 경로들(원래 순서).
    Ready(Vec<PathBuf>),
    /// 전송 시작/진행 중(새로 청한 파일 수).
    Started(usize),
    /// 합계가 상한을 넘는다 — 경로만.
    TooLarge { total: u64, max: u64 },
    /// 파일 내용 공유 꺼짐.
    Disabled,
    /// 원본 오프라인이고 캐시도 없다.
    Offline,
}

/// 펌프가 보낼 것 — (기기 hex, 메시지).
pub(crate) type Outgoing = (String, PeerMsg);

/// 관리자 — 인스턴스 기반(테스트가 임시 디렉터리로 하나 만든다) · 실행 시엔 [`MANAGER`] 하나.
pub(crate) struct Manager {
    list: Vec<Transfer>,
    next_req: u32,
    policy: Policy,
    cache_dir: PathBuf,
    /// ★ 이전 저장 폴더들(09-13 — 설정으로 바꾼 뒤에도 거기서 게시된 파일을 **우리 캐시**로 알아본다 ·
    ///   2PC 연쇄 가드 `is_cache_path`가 폴더 교체로 뚫리지 않게).
    prev_dirs: Vec<PathBuf>,
    /// 확인된 캐시(열쇠 → 최종 경로) — stat을 한 번만.
    cached: HashMap<String, PathBuf>,
    /// 붙여넣기가 기다리는 항목(하나) — 완료되면 셸이 게시·주입한다.
    pending_paste: Option<(u64, PasteAs, Instant)>,
    /// ★ 보내는 쪽 — 내가 제안한 경로들(파일 항목으로 전파한 것만 서빙 · 임의 읽기 차단).
    offered: HashSet<String>,
    /// 온라인 판정(테스트 주입).
    online: fn(&str) -> bool,
    /// 마지막 UI 통지 이후 진행이 있었나.
    dirty: bool,
}

impl Manager {
    pub(crate) fn new(cache_dir: PathBuf, online: fn(&str) -> bool) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        Self {
            list: Vec::new(),
            next_req: 1,
            policy: Policy::default(),
            cache_dir,
            prev_dirs: Vec::new(),
            cached: HashMap::new(),
            pending_paste: None,
            offered: HashSet::new(),
            online,
            dirty: false,
        }
    }

    pub(crate) fn set_policy(&mut self, p: Policy) {
        self.policy = p;
    }

    /// ★ 최신 항목 즉시 받기(09-13 사용자 — "다른 PC에서 복사한 파일이 바로 붙여넣기되지 않는다") —
    /// 이 항목의 진행 중 전송을 **Fg**로 올린다: 저속 상한을 받지 않고 다른 Bg는 양보한다.
    /// 상한 판정(`auto_bytes` · 0 = 끔)은 이미 [`Self::fetch`]가 Bg로 했으므로 자동 캐시 설정은 그대로 산다.
    /// 돌려주는 값 = 올린 전송 수.
    pub(crate) fn boost(&mut self, item_id: u64) -> usize {
        let now = Instant::now();
        let mut n = 0;
        for t in self.list.iter_mut().filter(|t| {
            t.item_id == item_id
                && matches!(t.state, State::Queued | State::Active | State::Offline)
        }) {
            if t.prio != Prio::Fg {
                t.prio = Prio::Fg;
                t.next_at = now;
                n += 1;
            }
        }
        if n > 0 {
            self.dirty = true;
        }
        n
    }

    pub(crate) fn policy(&self) -> Policy {
        self.policy
    }

    /// ★ 저장 폴더 교체(09-13 · `sync.file_dir`) — 이후 전송부터 새 폴더. 진행 중인 `.part`는 제 경로를
    /// 이미 들고 있어 영향이 없고, 옛 폴더는 [`Self::is_cache_path`]가 계속 알아본다. 캐시 메모는 비운다
    /// (열쇠→경로가 옛 폴더를 가리키므로 — 다음 조회가 새 폴더를 stat 해 미스면 다시 받는다).
    pub(crate) fn set_cache_dir(&mut self, dir: PathBuf) {
        if dir == self.cache_dir {
            return;
        }
        let _ = std::fs::create_dir_all(&dir);
        let old = std::mem::replace(&mut self.cache_dir, dir);
        if !self.prev_dirs.contains(&old) {
            self.prev_dirs.push(old);
        }
        self.cached.clear();
    }

    /// 지금 저장 폴더(테스트 확인용).
    #[cfg(test)]
    pub(crate) fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// 옛 폴더를 기억시킨다(부팅 — 설정 전 기본 위치 `<data>/cache/files`의 파일도 캐시로 판정).
    pub(crate) fn remember_dir(&mut self, dir: PathBuf) {
        if dir != self.cache_dir && !self.prev_dirs.contains(&dir) {
            self.prev_dirs.push(dir);
        }
    }

    // ───────────────────────────── 보내는 쪽

    /// 내가 전파한 파일 항목의 경로를 제안 목록에 올린다(상한 있음 — 오래된 것은 밀려난다).
    pub(crate) fn offer(&mut self, paths: &[String]) {
        if self.offered.len() > 20_000 {
            self.offered.clear();
        }
        for p in paths {
            self.offered.insert(p.clone());
        }
    }

    /// 블록 요청을 처리해 보낼 프레임들을 만든다(데이터 n개 + End, 또는 Err 하나).
    ///
    /// 세션 스레드에서 동기로 부른다 — 블록 상한이 [`FILE_BLOCK_MAX`]라 한 번에 수십 ms를 넘지 않는다.
    pub(crate) fn serve(&self, req: u32, path: &str, offset: u64, len: u32) -> Vec<PeerMsg> {
        let err = |code: FileErrCode, msg: &str| {
            vec![PeerMsg::FileErr {
                req,
                code,
                msg: msg.to_string(),
            }]
        };
        if !self.policy.contents {
            return err(FileErrCode::Disabled, "file contents sharing is off");
        }
        if !self.offered.contains(path) {
            return err(FileErrCode::NotOffered, "path was not offered");
        }
        let mut f = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => return err(FileErrCode::NotFound, &e.to_string()),
        };
        let meta = match f.metadata() {
            Ok(m) => m,
            Err(e) => return err(FileErrCode::Io, &e.to_string()),
        };
        let size = meta.len();
        let mtime = mtime_secs(&meta);
        let mut out = Vec::new();
        let want = u64::from(len.min(FILE_BLOCK_MAX));
        if offset < size && want > 0 {
            if let Err(e) = f.seek(std::io::SeekFrom::Start(offset)) {
                return err(FileErrCode::Io, &e.to_string());
            }
            let mut left = want.min(size - offset);
            let mut at = offset;
            let mut buf = vec![0u8; FILE_FRAME];
            while left > 0 {
                let n = (left as usize).min(FILE_FRAME);
                match f.read(&mut buf[..n]) {
                    Ok(0) => break,
                    Ok(k) => {
                        out.push(PeerMsg::FileData {
                            req,
                            offset: at,
                            data: buf[..k].to_vec(),
                        });
                        at += k as u64;
                        left -= k as u64;
                    }
                    Err(e) => return err(FileErrCode::Io, &e.to_string()),
                }
            }
        }
        out.push(PeerMsg::FileEnd { req, size, mtime });
        out
    }

    // ───────────────────────────── 캐시

    fn dir_of(&self, key: &str) -> PathBuf {
        self.cache_dir.join(key)
    }

    /// 캐시된 최종 경로(크기까지 맞을 때만) — 한 번 확인한 열쇠는 기억한다.
    pub(crate) fn cached_path(&mut self, origin_hex: &str, f: &RemoteFile) -> Option<PathBuf> {
        let key = cache_key(origin_hex, f);
        if let Some(p) = self.cached.get(&key) {
            return Some(p.clone());
        }
        let p = self.dir_of(&key).join(safe_name(&f.path));
        match std::fs::metadata(&p) {
            Ok(m) if m.is_file() && m.len() == f.size => {
                self.cached.insert(key, p.clone());
                Some(p)
            }
            _ => None,
        }
    }

    /// ★ 이 경로가 **우리 캐시 안**인가(09-12 연쇄 차단) — 캐시에서 게시한 파일을 감시가 되읽은 캡처는
    /// 남에게 보낼 것이 아니다(상대에겐 뜻 없는 경로이고, 보내면 상대가 또 캐시해 되돌려 보낸다).
    pub(crate) fn is_cache_path(&self, p: &str) -> bool {
        let norm = |s: &str| s.replace('/', "\\").to_lowercase();
        let q = norm(p);
        std::iter::once(&self.cache_dir)
            .chain(self.prev_dirs.iter())
            .any(|d| q.starts_with(&norm(&d.to_string_lossy())))
    }

    /// 항목의 파일 전부가 캐시돼 있으면 경로들.
    pub(crate) fn all_cached(&mut self, m: &RemoteFiles) -> Option<Vec<PathBuf>> {
        let mut out = Vec::with_capacity(m.files.len());
        for f in &m.files {
            out.push(self.cached_path(&m.origin_hex, f)?);
        }
        Some(out)
    }

    /// 용량 상한 — 오래된 디렉터리부터 지운다(전송 중·방금 쓴 것은 제외).
    fn evict(&mut self) {
        let cap = self.policy.cache_bytes;
        if cap == 0 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(&self.cache_dir) else {
            return;
        };
        let busy: HashSet<String> = self
            .list
            .iter()
            .filter(|t| !matches!(t.state, State::Done | State::Failed(_) | State::Stopped))
            .map(|t| t.key.clone())
            .collect();
        let mut dirs: Vec<(std::time::SystemTime, u64, PathBuf, String)> = Vec::new();
        let mut total = 0u64;
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let key = e.file_name().to_string_lossy().into_owned();
            let mut bytes = 0u64;
            let mut newest = std::time::SystemTime::UNIX_EPOCH;
            if let Ok(files) = std::fs::read_dir(&p) {
                for f in files.flatten() {
                    if let Ok(m) = f.metadata() {
                        bytes += m.len();
                        if let Ok(t) = m.modified() {
                            newest = newest.max(t);
                        }
                    }
                }
            }
            total += bytes;
            dirs.push((newest, bytes, p, key));
        }
        if total <= cap {
            return;
        }
        dirs.sort_by_key(|d| d.0);
        for (_, bytes, p, key) in dirs {
            if total <= cap {
                break;
            }
            if busy.contains(&key) {
                continue;
            }
            if std::fs::remove_dir_all(&p).is_ok() {
                total = total.saturating_sub(bytes);
                self.cached.remove(&key);
                println!("파일 캐시: {}MB 비움 — {key}", bytes / 1_048_576);
            }
        }
    }

    // ───────────────────────────── 받는 쪽 — 시작·제어

    /// 항목 상태(목록 배지) — stat은 열쇠당 한 번(메모).
    pub(crate) fn item_status(&mut self, m: &RemoteFiles) -> ItemStatus {
        if self.all_cached(m).is_some() {
            return ItemStatus::Cached;
        }
        let mine: Vec<&Transfer> = self.list.iter().filter(|t| t.item_id_matches(m)).collect();
        if mine.is_empty() {
            return ItemStatus::Remote;
        }
        if mine
            .iter()
            .any(|t| matches!(t.state, State::Queued | State::Active))
        {
            let total: u64 = m.total_bytes().max(1);
            let got: u64 = m
                .files
                .iter()
                .map(|f| {
                    let key = cache_key(&m.origin_hex, f);
                    if self.cached.contains_key(&key) {
                        f.size
                    } else {
                        mine.iter().find(|t| t.key == key).map_or(0, |t| t.received)
                    }
                })
                .sum();
            return ItemStatus::Fetching((got.saturating_mul(100) / total).min(100) as u8);
        }
        if mine.iter().any(|t| t.state == State::Offline) {
            return ItemStatus::Offline;
        }
        if mine.iter().any(|t| matches!(t.state, State::Failed(_))) {
            return ItemStatus::Failed;
        }
        ItemStatus::Remote
    }

    /// ★ 붙여넣기/사전 캐시 진입점 — 캐시되지 않은 파일마다 전송을 만든다(있으면 우선순위만 올린다).
    pub(crate) fn fetch(&mut self, item_id: u64, m: &RemoteFiles, prio: Prio) -> Fetch {
        if let Some(paths) = self.all_cached(m) {
            return Fetch::Ready(paths);
        }
        if !self.policy.contents {
            return Fetch::Disabled;
        }
        let total = m.total_bytes();
        let cap = match prio {
            Prio::Fg => self.policy.max_bytes,
            Prio::Bg => self.policy.auto_bytes,
        };
        if cap == 0 && prio == Prio::Bg {
            return Fetch::Disabled;
        }
        if cap > 0 && total > cap {
            return Fetch::TooLarge { total, max: cap };
        }
        let online = (self.online)(&m.origin_hex);
        if !online && prio == Prio::Fg {
            // 붙여넣기는 지금 답이 필요하다 — 오프라인이면 경로 텍스트로(docs/26 §4-2).
            return Fetch::Offline;
        }
        let mut started = 0usize;
        for f in &m.files {
            if self.cached_path(&m.origin_hex, f).is_some() {
                continue;
            }
            let key = cache_key(&m.origin_hex, f);
            if let Some(t) = self.list.iter_mut().find(|t| t.key == key) {
                // 이미 있다 — 붙여넣기면 승격 · 실패/중지는 다시 살린다.
                if prio == Prio::Fg {
                    t.prio = Prio::Fg;
                    t.item_id = item_id;
                }
                if matches!(t.state, State::Failed(_) | State::Stopped) {
                    t.state = if online {
                        State::Queued
                    } else {
                        State::Offline
                    };
                    t.retries = 0;
                    t.ended = None;
                    t.next_at = Instant::now();
                    started += 1;
                }
                continue;
            }
            let dir = self.dir_of(&key);
            let _ = std::fs::create_dir_all(&dir);
            let name = safe_name(&f.path);
            let final_path = dir.join(&name);
            let part_path = dir.join(format!("{name}.part"));
            // ★ 이어 받기 — .part 길이가 곧 다음 오프셋(크기를 넘었으면 버리고 처음부터).
            let mut received = std::fs::metadata(&part_path).map_or(0, |m| m.len());
            if received > f.size {
                let _ = std::fs::remove_file(&part_path);
                received = 0;
            }
            let now = Instant::now();
            let req = self.alloc_req();
            self.list.push(Transfer {
                req,
                item_id,
                origin_hex: m.origin_hex.clone(),
                origin_name: m.origin_name.clone(),
                file: f.clone(),
                key,
                prio,
                state: if online {
                    State::Queued
                } else {
                    State::Offline
                },
                received,
                part: None,
                part_path,
                final_path,
                next_at: now,
                last_io: now,
                retries: 0,
                ended: None,
                rate: 0.0,
                rate_mark: (now, received),
            });
            started += 1;
        }
        self.dirty = true;
        if started == 0 && self.all_cached(m).is_some() {
            return Fetch::Ready(self.all_cached(m).unwrap_or_default());
        }
        Fetch::Started(started)
    }

    fn alloc_req(&mut self) -> u32 {
        let r = self.next_req;
        self.next_req = self.next_req.wrapping_add(1).max(1);
        r
    }

    pub(crate) fn set_pending_paste(&mut self, item_id: u64, as_: PasteAs) {
        self.pending_paste = Some((item_id, as_, Instant::now()));
    }

    /// 항목이 끝났을 때 붙여넣기 대기를 꺼낸다 — `(모드, 시작한 지 얼마나)`.
    pub(crate) fn take_pending_paste(&mut self, item_id: u64) -> Option<(PasteAs, Duration)> {
        match self.pending_paste {
            Some((id, as_, at)) if id == item_id => {
                self.pending_paste = None;
                Some((as_, at.elapsed()))
            }
            _ => None,
        }
    }

    pub(crate) fn stop(&mut self, req: u32) -> bool {
        let Some(t) = self.list.iter_mut().find(|t| t.req == req) else {
            return false;
        };
        if matches!(t.state, State::Done | State::Failed(_) | State::Stopped) {
            return false;
        }
        t.state = State::Stopped;
        t.part = None;
        t.ended = Some(Instant::now());
        self.dirty = true;
        true
    }

    pub(crate) fn stop_item(&mut self, item_id: u64) -> usize {
        let reqs: Vec<u32> = self
            .list
            .iter()
            .filter(|t| t.item_id == item_id)
            .map(|t| t.req)
            .collect();
        reqs.into_iter().filter(|r| self.stop(*r)).count()
    }

    pub(crate) fn stop_all(&mut self) -> usize {
        let reqs: Vec<u32> = self.list.iter().map(|t| t.req).collect();
        reqs.into_iter().filter(|r| self.stop(*r)).count()
    }

    pub(crate) fn retry(&mut self, req: u32) -> bool {
        let online = self.online;
        let Some(t) = self.list.iter_mut().find(|t| t.req == req) else {
            return false;
        };
        if !matches!(t.state, State::Failed(_) | State::Stopped | State::Offline) {
            return false;
        }
        t.state = if online(&t.origin_hex) {
            State::Queued
        } else {
            State::Offline
        };
        t.retries = 0;
        t.ended = None;
        t.next_at = Instant::now();
        // 오프셋은 .part가 기억한다 — 이어 받는다.
        t.received = std::fs::metadata(&t.part_path).map_or(0, |m| m.len());
        self.dirty = true;
        true
    }

    /// 끝난 행(완료·실패·중지) 정리 — `older_than` 이상 지난 것만(패널 잔류).
    pub(crate) fn clear_finished(&mut self, older_than: Duration) -> usize {
        let before = self.list.len();
        self.list.retain(|t| {
            !(matches!(t.state, State::Done | State::Failed(_) | State::Stopped)
                && t.ended.is_some_and(|e| e.elapsed() >= older_than))
        });
        let n = before - self.list.len();
        if n > 0 {
            self.dirty = true;
        }
        n
    }

    /// 항목 삭제 — 그 항목의 전송을 지운다(캐시는 둔다 · 용량 정책이 정리).
    pub(crate) fn forget_item(&mut self, item_id: u64) {
        self.stop_item(item_id);
        self.list.retain(|t| t.item_id != item_id);
    }

    // ───────────────────────────── 받는 쪽 — 펌프·데이터

    /// 펌프 한 바퀴 — 청할 블록을 정하고, 타임아웃·오프라인 전이를 처리한다.
    pub(crate) fn tick(&mut self) -> Vec<Outgoing> {
        let now = Instant::now();
        let online = self.online;
        let mut out = Vec::new();
        // ① 오프라인 ↔ 대기 전이(원본 세션 유무).
        for t in &mut self.list {
            let on = online(&t.origin_hex);
            match t.state {
                State::Queued | State::Active if !on => {
                    t.state = State::Offline;
                    t.part = None;
                    self.dirty = true;
                }
                State::Offline if on => {
                    t.state = State::Queued;
                    t.next_at = now;
                    self.dirty = true;
                }
                _ => {}
            }
        }
        // ② 타임아웃 — 같은 오프셋 재요청(상한 뒤 실패).
        for t in &mut self.list {
            if t.state == State::Active && now.duration_since(t.last_io) > BLOCK_TIMEOUT {
                t.retries += 1;
                if t.retries > MAX_RETRY {
                    t.state = State::Failed("no response from origin".into());
                    t.part = None;
                    t.ended = Some(now);
                } else {
                    t.state = State::Queued;
                    t.next_at = now;
                }
                self.dirty = true;
            }
        }
        // ③ 청하기 — Fg 먼저(동시 2) · Fg가 없을 때만 Bg 하나.
        let fg_active = self
            .list
            .iter()
            .filter(|t| t.prio == Prio::Fg && t.state == State::Active)
            .count();
        let fg_pending = self
            .list
            .iter()
            .any(|t| t.prio == Prio::Fg && matches!(t.state, State::Queued | State::Active));
        let bg_active = self
            .list
            .iter()
            .any(|t| t.prio == Prio::Bg && t.state == State::Active);
        let mut fg_slots = FG_INFLIGHT.saturating_sub(fg_active);
        let mut bg_slot = !fg_pending && !bg_active;
        for t in &mut self.list {
            if t.state != State::Queued || t.next_at > now {
                continue;
            }
            let block = match t.prio {
                Prio::Fg if fg_slots > 0 => {
                    fg_slots -= 1;
                    FG_BLOCK
                }
                Prio::Bg if bg_slot => {
                    bg_slot = false;
                    BG_BLOCK
                }
                _ => continue,
            };
            t.state = State::Active;
            t.last_io = now;
            out.push((
                t.origin_hex.clone(),
                PeerMsg::FileReq {
                    req: t.req,
                    offset: t.received,
                    len: block,
                    path: t.file.path.clone(),
                },
            ));
        }
        out
    }

    /// 데이터 프레임 — 연속 오프셋만 받아 `.part`에 붙인다.
    pub(crate) fn on_data(&mut self, hex: &str, req: u32, offset: u64, data: &[u8]) {
        let Some(t) = self
            .list
            .iter_mut()
            .find(|t| t.req == req && t.origin_hex == hex)
        else {
            return;
        };
        if t.state != State::Active || offset != t.received {
            return; // 늦게 온 조각(중지 뒤·재요청 뒤) — 버린다.
        }
        if t.part.is_none() {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&t.part_path);
            match f {
                Ok(f) => t.part = Some(f),
                Err(e) => {
                    t.state = State::Failed(format!("cache write: {e}"));
                    t.ended = Some(Instant::now());
                    self.dirty = true;
                    return;
                }
            }
        }
        if let Some(f) = t.part.as_mut() {
            if let Err(e) = f.write_all(data) {
                t.state = State::Failed(format!("cache write: {e}"));
                t.part = None;
                t.ended = Some(Instant::now());
                self.dirty = true;
                return;
            }
        }
        t.received += data.len() as u64;
        let now = Instant::now();
        t.last_io = now;
        let (mark_at, mark_bytes) = t.rate_mark;
        let dt = now.duration_since(mark_at).as_secs_f64();
        if dt >= 0.5 {
            let inst = (t.received - mark_bytes) as f64 / dt;
            t.rate = if t.rate == 0.0 {
                inst
            } else {
                t.rate * 0.6 + inst * 0.4
            };
            t.rate_mark = (now, t.received);
        }
        self.dirty = true;
    }

    /// 블록 끝 — 완료면 실체화, 아니면 다음 블록 예약. 항목이 **전부** 끝났으면 `Some(item_id)`.
    pub(crate) fn on_end(&mut self, hex: &str, req: u32, size: u64, mtime: u64) -> Option<u64> {
        let bg_bps = self.policy.bg_bps;
        let t = self
            .list
            .iter_mut()
            .find(|t| t.req == req && t.origin_hex == hex)?;
        if t.state != State::Active {
            return None;
        }
        let now = Instant::now();
        t.last_io = now;
        t.retries = 0;
        if size != t.file.size || (mtime != t.file.mtime && t.file.mtime != 0) {
            // 원본이 바뀌었다 — 반쯤 받은 것을 이어 붙이면 깨진 파일이 된다.
            t.state = State::Failed("source file changed since it was copied".into());
            t.part = None;
            let _ = std::fs::remove_file(&t.part_path);
            t.received = 0;
            t.ended = Some(now);
            self.dirty = true;
            return self.item_settled(req);
        }
        if t.received >= t.file.size {
            if let Some(f) = t.part.take() {
                let _ = f.sync_all();
            }
            if t.file.size == 0 {
                let _ = std::fs::File::create(&t.part_path);
            }
            match std::fs::rename(&t.part_path, &t.final_path) {
                Ok(()) => {
                    t.state = State::Done;
                    self.cached.insert(t.key.clone(), t.final_path.clone());
                }
                Err(e) => {
                    t.state = State::Failed(format!("cache finalize: {e}"));
                }
            }
            t.ended = Some(now);
            self.dirty = true;
            self.evict();
            return self.item_settled(req);
        }
        // 다음 블록 — Bg는 속도 상한만큼 쉰다(방금 받은 블록 크기 기준).
        t.state = State::Queued;
        t.next_at = if t.prio == Prio::Bg && bg_bps > 0 {
            now + Duration::from_secs_f64(f64::from(BG_BLOCK) / bg_bps as f64)
        } else {
            now
        };
        self.dirty = true;
        None
    }

    pub(crate) fn on_err(
        &mut self,
        hex: &str,
        req: u32,
        code: FileErrCode,
        msg: &str,
    ) -> Option<u64> {
        let t = self
            .list
            .iter_mut()
            .find(|t| t.req == req && t.origin_hex == hex)?;
        t.state = State::Failed(format!("{code:?}: {msg}"));
        t.part = None;
        t.ended = Some(Instant::now());
        self.dirty = true;
        self.item_settled(req)
    }

    /// `req`가 속한 항목의 전송이 전부 종결됐으면 항목 id.
    fn item_settled(&self, req: u32) -> Option<u64> {
        let item_id = self.list.iter().find(|t| t.req == req)?.item_id;
        let unsettled = self.list.iter().any(|t| {
            t.item_id == item_id
                && !matches!(t.state, State::Done | State::Failed(_) | State::Stopped)
        });
        (!unsettled).then_some(item_id)
    }

    /// 항목의 전송이 전부 `Done`인가.
    pub(crate) fn item_done(&self, item_id: u64) -> bool {
        let mine: Vec<&Transfer> = self.list.iter().filter(|t| t.item_id == item_id).collect();
        !mine.is_empty() && mine.iter().all(|t| t.state == State::Done)
    }

    /// 항목의 첫 실패 사유.
    pub(crate) fn item_failure(&self, item_id: u64) -> Option<String> {
        self.list
            .iter()
            .find_map(|t| match (&t.state, t.item_id == item_id) {
                (State::Failed(m), true) => Some(m.clone()),
                (State::Stopped, true) => Some("stopped".into()),
                _ => None,
            })
    }

    // ───────────────────────────── 조회

    pub(crate) fn views(&self) -> Vec<XferView> {
        self.list
            .iter()
            .map(|t| XferView {
                req: t.req,
                item_id: t.item_id,
                name: t.file.name(),
                origin_name: t.origin_name.clone(),
                size: t.file.size,
                received: t.received,
                state: t.state.clone(),
                prio: t.prio,
                rate: t.rate,
            })
            .collect()
    }

    /// 진행 중 요약 — `(파일 수, 합계 진행률)`. 없으면 `None`.
    pub(crate) fn active_summary(&self) -> Option<(usize, u8)> {
        let act: Vec<&Transfer> = self
            .list
            .iter()
            .filter(|t| matches!(t.state, State::Queued | State::Active))
            .collect();
        if act.is_empty() {
            return None;
        }
        let total: u64 = act.iter().map(|t| t.file.size).sum::<u64>().max(1);
        let got: u64 = act.iter().map(|t| t.received).sum();
        Some((act.len(), (got.saturating_mul(100) / total).min(100) as u8))
    }

    pub(crate) fn has_active(&self) -> bool {
        self.list
            .iter()
            .any(|t| matches!(t.state, State::Queued | State::Active))
    }

    /// UI 통지가 필요한가(진행이 있었나) — 읽으면 내린다.
    pub(crate) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

impl Transfer {
    fn item_id_matches(&self, m: &RemoteFiles) -> bool {
        self.origin_hex == m.origin_hex
            && m.files
                .iter()
                .any(|f| cache_key(&m.origin_hex, f) == self.key)
    }
}

/// 수정시각(unix 초 · 없으면 0).
pub(crate) fn mtime_secs(m: &std::fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// 캐시 파일 이름 — 경로의 마지막 조각에서 OS가 거부하는 글자만 바꾼다(이름은 네트워크에서 왔다).
fn safe_name(path: &str) -> String {
    let base = nclip_core::capture::base_name(path);
    let mut s: String = base
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    s = s.trim_matches([' ', '.']).to_string();
    if s.is_empty() {
        s = "file".into();
    }
    if s.len() > 200 {
        let mut end = 200;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

// ───────────────────────────── 전역(실행 시) 래퍼

static MANAGER: Mutex<Option<Manager>> = Mutex::new(None);

/// 시작 때 한 번 — 저장 폴더 = [`resolve_file_dir`]. `legacy`(설정 도입 전 위치 `<data>/cache/files`)가
/// 실재하면 기억해 둔다 — 거기서 게시된 파일도 우리 캐시로 판정(2PC 연쇄 가드).
pub(crate) fn init(cache_dir: PathBuf, legacy: Option<PathBuf>) {
    let mut m = Manager::new(cache_dir, crate::devices::is_online);
    if let Some(l) = legacy.filter(|l| l.is_dir()) {
        m.remember_dir(l);
    }
    if let Ok(mut g) = MANAGER.lock() {
        *g = Some(m);
    }
}

/// ★ 저장 폴더 교체(09-13 · 설정 창이 `sync.file_dir` 변경 시).
pub(crate) fn set_cache_dir(dir: PathBuf) {
    let _ = with(|m| m.set_cache_dir(dir));
}

/// 프로그램 폴더 이름 — 다운로드 폴더 아래 이 이름으로 만든다.
pub(crate) const FILE_DIR_NAME: &str = "Nexa Clip";

/// ★ `sync.file_dir` → 실제 저장 폴더(09-13 사용자 — "기본은 운영체제의 사용자 다운로드 폴더").
///
/// | 설정값 | 결과 |
/// |---|---|
/// | 비어 있지 않음 | 그 경로(앞의 `~`는 홈으로) |
/// | 비어 있음 · OS 다운로드 폴더 있음 | `<Downloads>/Nexa Clip` |
/// | 비어 있음 · 다운로드 폴더 없음 | `<data>/cache/files`(종전 위치) |
pub(crate) fn resolve_file_dir(setting: &str, data_dir: &Path) -> PathBuf {
    let v = setting.trim();
    if !v.is_empty() {
        if let Some(rest) = v.strip_prefix('~') {
            if let Some(h) = nclip_plat::paths::home_dir() {
                return h.join(rest.trim_start_matches(['/', '\\']));
            }
        }
        return PathBuf::from(v);
    }
    nclip_plat::paths::downloads_dir().map_or_else(
        || data_dir.join("cache").join("files"),
        |d| d.join(FILE_DIR_NAME),
    )
}

/// 관리자에 접근(초기화 전이면 기본값/무시).
pub(crate) fn with<R>(f: impl FnOnce(&mut Manager) -> R) -> Option<R> {
    let mut g = MANAGER.lock().ok()?;
    g.as_mut().map(f)
}

pub(crate) fn set_policy(p: Policy) {
    let _ = with(|m| m.set_policy(p));
}

/// 경로 전부가 우리 캐시 안인가(비면 `false`).
pub(crate) fn all_cache_paths(paths: &[String]) -> bool {
    !paths.is_empty() && with(|m| paths.iter().all(|p| m.is_cache_path(p))).unwrap_or(false)
}

pub(crate) fn offer(paths: &[String]) {
    let _ = with(|m| m.offer(paths));
}

pub(crate) fn views() -> Vec<XferView> {
    with(|m| m.views()).unwrap_or_default()
}

pub(crate) fn active_summary() -> Option<(usize, u8)> {
    with(|m| m.active_summary()).flatten()
}

pub(crate) fn has_active() -> bool {
    with(|m| m.has_active()).unwrap_or(false)
}

/// ★ 펌프 스레드 — 100ms마다 [`Manager::tick`] → 세션 채널로 `FileReq` · 진행이 있었으면 UI를 깨운다.
pub(crate) fn spawn_pump(proxy: winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>) {
    let _ = std::thread::Builder::new()
        .name("nclip-xfer-pump".into())
        .spawn(move || {
            let mut last_ui = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(100));
                let (out, dirty) = with(|m| {
                    let out = m.tick();
                    let n = m.clear_finished(LINGER.saturating_mul(4));
                    (out, m.take_dirty() || n > 0)
                })
                .unwrap_or_default();
                for (hex, msg) in out {
                    if !crate::sync_cmd::send_file(&hex, msg) {
                        // 세션이 없다 — 다음 tick의 오프라인 전이가 처리한다.
                    }
                }
                if dirty && last_ui.elapsed() >= Duration::from_millis(250) {
                    last_ui = Instant::now();
                    let _ = proxy.send_event(crate::tray_cmd::ShellEvent::XferTick);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nclip-xfer-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&d).expect("tmp");
        d
    }

    fn online(_: &str) -> bool {
        true
    }
    fn offline(_: &str) -> bool {
        false
    }

    /// 보내는 쪽 파일 + 매니페스트.
    fn source(dir: &Path, name: &str, bytes: usize) -> (String, RemoteFiles) {
        let p = dir.join(name);
        let data: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &data).expect("write");
        let meta = std::fs::metadata(&p).expect("meta");
        let path = p.to_string_lossy().into_owned();
        let m = RemoteFiles {
            origin_hex: "ab".repeat(32),
            origin_name: "A".into(),
            files: vec![RemoteFile {
                path: path.clone(),
                size: meta.len(),
                mtime: mtime_secs(&meta),
            }],
        };
        (path, m)
    }

    /// 펌프 + 세션을 흉내 낸다 — 받는 쪽 tick의 요청을 보내는 쪽 serve에 넣고 답을 되돌린다.
    fn pump(rx: &mut Manager, tx: &Manager, hex: &str, max_rounds: usize) -> Option<u64> {
        let mut settled = None;
        for _ in 0..max_rounds {
            let out = rx.tick();
            if out.is_empty() {
                if !rx.has_active() {
                    break;
                }
                continue;
            }
            for (_, msg) in out {
                let PeerMsg::FileReq {
                    req,
                    offset,
                    len,
                    path,
                } = msg
                else {
                    panic!("tick은 FileReq만 낸다")
                };
                for reply in tx.serve(req, &path, offset, len) {
                    match reply {
                        PeerMsg::FileData { req, offset, data } => {
                            rx.on_data(hex, req, offset, &data)
                        }
                        PeerMsg::FileEnd { req, size, mtime } => {
                            if let Some(id) = rx.on_end(hex, req, size, mtime) {
                                settled = Some(id);
                            }
                        }
                        PeerMsg::FileErr { req, code, msg } => {
                            if let Some(id) = rx.on_err(hex, req, code, &msg) {
                                settled = Some(id);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        settled
    }

    /// ★ 전 구간 — 요청 → 블록 여러 개 → 실체화 → 캐시 적중 → 내용 동일.
    #[test]
    fn fetch_streams_blocks_and_materializes_into_cache() {
        let src = tmp("src");
        let (path, m) = source(&src, "보고서 초안.bin", 5 * 1024 * 1024 + 123);
        let mut tx = Manager::new(tmp("txc"), online);
        tx.offer(std::slice::from_ref(&path));
        let mut rx = Manager::new(tmp("rxc"), online);
        assert_eq!(rx.item_status(&m), ItemStatus::Remote);
        assert_eq!(rx.fetch(7, &m, Prio::Fg), Fetch::Started(1));
        assert!(matches!(rx.item_status(&m), ItemStatus::Fetching(_)));
        let settled = pump(&mut rx, &tx, &m.origin_hex, 100);
        assert_eq!(settled, Some(7));
        assert!(rx.item_done(7));
        let Fetch::Ready(paths) = rx.fetch(7, &m, Prio::Fg) else {
            panic!("캐시 적중이어야 한다")
        };
        assert_eq!(paths.len(), 1);
        assert_eq!(
            std::fs::read(&paths[0]).expect("cached"),
            std::fs::read(&path).expect("src"),
            "내용 동일"
        );
        assert!(paths[0].ends_with("보고서 초안.bin"));
        assert_eq!(rx.item_status(&m), ItemStatus::Cached);
        // 새 관리자(재시작)도 디스크의 캐시를 알아본다.
        let mut again = Manager::new(rx.cache_dir.clone(), online);
        assert!(matches!(again.fetch(7, &m, Prio::Fg), Fetch::Ready(_)));
    }

    /// ★ 이어 받기 — 중간에 중지했다가 재시도하면 `.part` 길이부터 청한다.
    #[test]
    fn resume_continues_from_partial_offset() {
        let src = tmp("src2");
        let (path, m) = source(&src, "big.bin", 3 * 1024 * 1024);
        let mut tx = Manager::new(tmp("txc2"), online);
        tx.offer(std::slice::from_ref(&path));
        let mut rx = Manager::new(tmp("rxc2"), online);
        rx.fetch(1, &m, Prio::Fg);
        // 첫 블록(2MiB)만 처리하고 중지.
        let out = rx.tick();
        let (
            _,
            PeerMsg::FileReq {
                req,
                offset,
                len,
                path: p,
            },
        ) = out.into_iter().next().expect("req")
        else {
            panic!()
        };
        assert_eq!(offset, 0);
        for reply in tx.serve(req, &p, offset, len) {
            match reply {
                PeerMsg::FileData { req, offset, data } => {
                    rx.on_data(&m.origin_hex, req, offset, &data)
                }
                PeerMsg::FileEnd { req, size, mtime } => {
                    rx.on_end(&m.origin_hex, req, size, mtime);
                }
                _ => {}
            }
        }
        assert!(rx.stop(req));
        let v = rx.views();
        assert_eq!(v[0].state, State::Stopped);
        assert_eq!(v[0].received, 2 * 1024 * 1024);
        // 재시도 → 다음 요청 오프셋 = 2MiB.
        assert!(rx.retry(req));
        let out = rx.tick();
        let (_, PeerMsg::FileReq { offset, .. }) = &out[0] else {
            panic!()
        };
        assert_eq!(*offset, 2 * 1024 * 1024, "이어 받기");
        // 마저 받는다.
        for (_, msg) in out {
            let PeerMsg::FileReq {
                req,
                offset,
                len,
                path,
            } = msg
            else {
                panic!()
            };
            for reply in tx.serve(req, &path, offset, len) {
                match reply {
                    PeerMsg::FileData { req, offset, data } => {
                        rx.on_data(&m.origin_hex, req, offset, &data)
                    }
                    PeerMsg::FileEnd { req, size, mtime } => {
                        rx.on_end(&m.origin_hex, req, size, mtime);
                    }
                    _ => {}
                }
            }
        }
        assert!(rx.item_done(1));
        assert_eq!(
            std::fs::read(rx.all_cached(&m).expect("cached")[0].clone()).expect("read"),
            std::fs::read(&path).expect("src")
        );
    }

    /// 제안하지 않은 경로·꺼진 정책은 서빙하지 않는다(임의 읽기 차단).
    #[test]
    fn serve_refuses_unoffered_and_disabled() {
        let src = tmp("src3");
        let (path, _) = source(&src, "x.bin", 10);
        let mut tx = Manager::new(tmp("txc3"), online);
        let r = tx.serve(1, &path, 0, 100);
        assert!(matches!(
            r[0],
            PeerMsg::FileErr {
                code: FileErrCode::NotOffered,
                ..
            }
        ));
        tx.offer(std::slice::from_ref(&path));
        assert!(matches!(
            tx.serve(1, &path, 0, 100)[0],
            PeerMsg::FileData { .. }
        ));
        tx.set_policy(Policy {
            contents: false,
            ..Policy::default()
        });
        assert!(matches!(
            tx.serve(1, &path, 0, 100)[0],
            PeerMsg::FileErr {
                code: FileErrCode::Disabled,
                ..
            }
        ));
        tx.set_policy(Policy::default());
        assert!(matches!(
            tx.serve(2, &src.join("없음.bin").to_string_lossy(), 0, 1)[0],
            PeerMsg::FileErr {
                code: FileErrCode::NotOffered,
                ..
            }
        ));
    }

    /// 상한·오프라인·끔 판정은 **한 바이트도 흐르기 전**(docs/26 §4-4).
    #[test]
    fn fetch_refuses_before_any_byte_flows() {
        let src = tmp("src4");
        let (_, m) = source(&src, "y.bin", 2048);
        let mut rx = Manager::new(tmp("rxc4"), online);
        rx.set_policy(Policy {
            max_bytes: 1024,
            auto_bytes: 512,
            ..Policy::default()
        });
        assert_eq!(
            rx.fetch(1, &m, Prio::Fg),
            Fetch::TooLarge {
                total: 2048,
                max: 1024
            }
        );
        assert_eq!(
            rx.fetch(1, &m, Prio::Bg),
            Fetch::TooLarge {
                total: 2048,
                max: 512
            }
        );
        rx.set_policy(Policy {
            contents: false,
            ..Policy::default()
        });
        assert_eq!(rx.fetch(1, &m, Prio::Fg), Fetch::Disabled);
        rx.set_policy(Policy {
            auto_bytes: 0,
            ..Policy::default()
        });
        assert_eq!(
            rx.fetch(1, &m, Prio::Bg),
            Fetch::Disabled,
            "0 = 사전 캐시 끔"
        );
        let mut off = Manager::new(tmp("rxc4b"), offline);
        assert_eq!(off.fetch(1, &m, Prio::Fg), Fetch::Offline);
        // Bg는 오프라인이어도 줄을 서고(원본이 돌아오면 자동), 배지는 Offline.
        assert_eq!(off.fetch(1, &m, Prio::Bg), Fetch::Started(1));
        assert_eq!(off.item_status(&m), ItemStatus::Offline);
        assert!(off.tick().is_empty(), "오프라인엔 청하지 않는다");
    }

    /// Bg는 Fg에 양보하고, 블록 사이에 속도 상한만큼 쉰다.
    #[test]
    fn background_yields_to_foreground_and_paces() {
        let src = tmp("src5");
        let (pa, ma) = source(&src, "a.bin", 600 * 1024);
        let (pb, mb) = source(&src, "b.bin", 600 * 1024);
        let mut tx = Manager::new(tmp("txc5"), online);
        tx.offer(&[pa, pb]);
        let mut rx = Manager::new(tmp("rxc5"), online);
        rx.set_policy(Policy {
            bg_bps: 1024 * 1024,
            ..Policy::default()
        });
        rx.fetch(1, &ma, Prio::Bg);
        rx.fetch(2, &mb, Prio::Fg);
        let out = rx.tick();
        assert_eq!(out.len(), 1, "Fg가 있으면 Bg는 청하지 않는다");
        let (
            _,
            PeerMsg::FileReq {
                len,
                req,
                path,
                offset,
            },
        ) = out.into_iter().next().expect("fg")
        else {
            panic!()
        };
        assert_eq!(len, FG_BLOCK);
        for reply in tx.serve(req, &path, offset, len) {
            match reply {
                PeerMsg::FileData { req, offset, data } => {
                    rx.on_data(&mb.origin_hex, req, offset, &data)
                }
                PeerMsg::FileEnd { req, size, mtime } => {
                    rx.on_end(&mb.origin_hex, req, size, mtime);
                }
                _ => {}
            }
        }
        assert!(rx.item_done(2));
        // 이제 Bg 차례 — 256KiB 블록, 블록 뒤 next_at이 미래(속도 상한).
        let out = rx.tick();
        assert_eq!(out.len(), 1);
        let (
            _,
            PeerMsg::FileReq {
                len,
                req,
                path,
                offset,
            },
        ) = out.into_iter().next().expect("bg")
        else {
            panic!()
        };
        assert_eq!(len, BG_BLOCK);
        for reply in tx.serve(req, &path, offset, len) {
            match reply {
                PeerMsg::FileData { req, offset, data } => {
                    rx.on_data(&ma.origin_hex, req, offset, &data)
                }
                PeerMsg::FileEnd { req, size, mtime } => {
                    rx.on_end(&ma.origin_hex, req, size, mtime);
                }
                _ => {}
            }
        }
        assert!(
            rx.tick().is_empty(),
            "속도 상한 대기 중엔 다음 블록을 청하지 않는다"
        );
        assert_eq!(rx.views()[0].state, State::Queued);
        assert!(matches!(rx.item_status(&ma), ItemStatus::Fetching(p) if p > 0 && p < 100));
    }

    /// 원본이 바뀌면(크기 불일치) 반쯤 받은 것을 버리고 실패로 — 깨진 파일을 만들지 않는다.
    #[test]
    fn source_change_fails_instead_of_corrupting() {
        let src = tmp("src6");
        let (path, mut m) = source(&src, "c.bin", 1000);
        m.files[0].size = 999; // 복사 시점 메타가 다르다고 가정
        let mut tx = Manager::new(tmp("txc6"), online);
        tx.offer(&[path]);
        let mut rx = Manager::new(tmp("rxc6"), online);
        rx.fetch(3, &m, Prio::Fg);
        let settled = pump(&mut rx, &tx, &m.origin_hex, 10);
        assert_eq!(settled, Some(3));
        assert!(matches!(rx.views()[0].state, State::Failed(_)));
        assert_eq!(rx.item_status(&m), ItemStatus::Failed);
        assert!(rx.item_failure(3).is_some());
        assert!(!rx.views()[0].name.is_empty());
    }

    /// 캐시 용량 상한 — 오래된 디렉터리부터 비운다.
    #[test]
    fn cache_evicts_oldest_past_cap() {
        let src = tmp("src7");
        let (pa, ma) = source(&src, "old.bin", 4096);
        let (pb, mb) = source(&src, "new.bin", 4096);
        let mut tx = Manager::new(tmp("txc7"), online);
        tx.offer(&[pa, pb]);
        let mut rx = Manager::new(tmp("rxc7"), online);
        rx.set_policy(Policy {
            cache_bytes: 6000,
            ..Policy::default()
        });
        rx.fetch(1, &ma, Prio::Fg);
        pump(&mut rx, &tx, &ma.origin_hex, 10);
        std::thread::sleep(Duration::from_millis(1100)); // mtime 초 단위 차이
        rx.fetch(2, &mb, Prio::Fg);
        pump(&mut rx, &tx, &mb.origin_hex, 10);
        assert!(rx.all_cached(&mb).is_some(), "새 것은 남는다");
        let mut fresh = Manager::new(rx.cache_dir.clone(), online);
        assert!(fresh.all_cached(&ma).is_none(), "오래된 것이 비워졌다");
    }

    /// ★ 최신 항목 즉시 받기(09-13) — Bg로 줄 선 전송을 `boost`하면 Fg가 되어 속도 상한(여기선 1 B/s)에
    ///   걸리지 않고 끝난다. 상한 판정은 fetch 시점의 Bg 규칙 그대로(자동 캐시 0이면 애초에 안 선다).
    #[test]
    fn boost_lifts_background_to_foreground() {
        let dir = tmp("boost");
        let (_, m) = source(&dir, "new.bin", 300 * 1024);
        let mut tx = Manager::new(dir.join("tx"), online);
        tx.offer(&m.paths());
        let mut rx = Manager::new(dir.join("rx"), online);
        rx.set_policy(Policy {
            bg_bps: 1,
            ..Policy::default()
        });
        assert_eq!(rx.fetch(9, &m, Prio::Bg), Fetch::Started(1));
        assert_eq!(rx.boost(9), 1);
        assert_eq!(rx.boost(9), 0, "이미 Fg면 0");
        assert!(rx.views().iter().all(|v| v.prio == Prio::Fg));
        assert_eq!(
            pump(&mut rx, &tx, &m.origin_hex, 200),
            Some(9),
            "저속 상한 없이 끝난다"
        );
        assert!(matches!(rx.fetch(9, &m, Prio::Fg), Fetch::Ready(_)));
        let mut off = Manager::new(dir.join("off"), online);
        off.set_policy(Policy {
            auto_bytes: 0,
            ..Policy::default()
        });
        assert_eq!(off.fetch(9, &m, Prio::Bg), Fetch::Disabled);
        assert_eq!(off.boost(9), 0, "자동 캐시 끔이면 올릴 전송이 없다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ★ 저장 폴더 교체(09-13) — 새 폴더로 실체화되고, 옛 폴더의 파일도 여전히 **우리 캐시**로 판정된다
    ///   (설정을 바꾼 뒤 옛 캐시를 붙여넣어도 2PC 연쇄가 다시 열리지 않는다).
    #[test]
    fn changing_dir_keeps_recognizing_old_dir() {
        let a = tmp("dirA");
        let b = tmp("dirB");
        let mut m = Manager::new(a.clone(), online);
        let in_a = a.join("k").join("x.txt").to_string_lossy().into_owned();
        m.set_cache_dir(b.clone());
        assert_eq!(m.cache_dir(), b.as_path());
        assert!(b.is_dir(), "새 폴더는 만들어진다");
        assert!(m.is_cache_path(&in_a), "옛 폴더도 캐시");
        let in_b = b.join("k").join("x.txt").to_string_lossy().into_owned();
        assert!(m.is_cache_path(&in_b));
        assert!(!m.is_cache_path("/elsewhere/x.txt"));
        // 같은 폴더로 다시 = 무시(이전 목록이 늘지 않는다).
        m.set_cache_dir(b.clone());
        assert_eq!(m.prev_dirs.len(), 1);
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    /// ★ 설정값 해석(09-13) — 값이 있으면 그대로(`~` 전개) · 비면 다운로드 폴더 아래 프로그램 폴더 또는 데이터 폴더.
    #[test]
    fn resolve_file_dir_prefers_setting_then_downloads() {
        let data = tmp("data");
        assert_eq!(
            resolve_file_dir("  /srv/inbox  ", &data),
            PathBuf::from("/srv/inbox")
        );
        if let Some(h) = nclip_plat::paths::home_dir() {
            assert_eq!(resolve_file_dir("~/받기", &data), h.join("받기"));
        }
        let d = resolve_file_dir("", &data);
        match nclip_plat::paths::downloads_dir() {
            Some(dl) => assert_eq!(d, dl.join(FILE_DIR_NAME)),
            None => assert_eq!(d, data.join("cache").join("files")),
        }
    }

    /// 캐시 안 경로 판정 — 구분자·대소문자 차이를 무시한다(Windows 경로).
    #[test]
    fn cache_path_detection_ignores_separator_and_case() {
        let dir = tmp("cp");
        let m = Manager::new(dir.clone(), online);
        let inside = dir.join("abc").join("x.txt").to_string_lossy().into_owned();
        assert!(m.is_cache_path(&inside));
        assert!(m.is_cache_path(&inside.replace('\\', "/").to_uppercase()));
        assert!(!m.is_cache_path("/somewhere/else/x.txt"));
    }

    #[test]
    fn safe_names_strip_reserved_characters() {
        assert_eq!(safe_name("D:\\a\\b:c*d?.txt"), "b_c_d_.txt");
        assert_eq!(safe_name("/tmp/"), "tmp");
        assert_eq!(safe_name(""), "file");
        assert_eq!(safe_name("   ..."), "file");
    }
}
