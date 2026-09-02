//! Linux 클립보드 **직접 구현** — x11rb + XFIXES (T-14 본편 · [docs/29 §6](../../../docs/29-linux-clipboard-access.md)).
//!
//! 도구 파이프 1단(`wl-paste`/`xclip` — [`crate::watch_linux`])을 대체한다. 외부 도구 0 ·
//! 새 crate 0(x11rb는 K-1이 이미 들여왔다 — `xfixes` feature만 추가). Wayland 세션에서는
//! XWayland로 붙는다(Mutter가 X11↔Wayland 셀렉션을 동기화 — xclip이 동작하던 것과 같은 원리).
//!
//! ## 연결 3개 — 하나로 합치면 자기 셀렉션을 자기가 읽을 때 교착한다(x11-clipboard 선례)
//!
//! | 연결 | 역할 | 스레드 |
//! |---|---|---|
//! | 감시 | `XFixesSelectSelectionInput` — 소유자 변경 **이벤트 푸시**(폴링 소멸 · DR-9) | 전용(유휴 = 블록) |
//! | 읽기 | `ConvertSelection` + INCR 수신 | 호출자(Mutex 직렬화 · 트랜잭션 동안만 펌프) |
//! | 서빙 | `SetSelectionOwner` + `SelectionRequest` 응대 + INCR 송신 | 전용(첫 게시 때 기동) |
//!
//! ## CopyQ 방어층 수용(29 §4)
//!
//! 변환 실패(property=None) = 정직한 실패 · 자기 게시 에코는 **소유자 창 비교**로 원천 차단 ·
//! 상한은 **전송 도중** 집행(INCR 청크 단위 — 다 받고 버리면 늦다).
//!
//! ## 정직 강등
//!
//! X 연결·XFIXES 부재는 이유와 함께 실패한다 — 호출자([`crate::watch_linux`] 사다리)가
//! 도구 파이프로 강등한다. 조용한 빈 목록은 없다(docs/02 R-4).

use nclip_core::RawRep;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use x11rb::connection::Connection as _;
use x11rb::protocol::xfixes::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _,
    CreateWindowAux, EventMask, PropMode, Property, SelectionNotifyEvent, SelectionRequestEvent,
    Window, WindowClass, CLIENT_MESSAGE_EVENT, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

/// 한 변환(SelectionNotify·INCR 청크)을 기다리는 상한 — 소유자가 죽었으면 여기서 끊는다.
const READ_TIMEOUT: Duration = Duration::from_millis(2000);
/// 게시 확인(ack) 상한 — 서빙 스레드가 소유권을 잡았다고 답할 때까지.
const ACK_TIMEOUT: Duration = Duration::from_millis(1000);
/// INCR 송신 청크 — 코어 프로토콜 최소 최대요청(256KB)보다 넉넉히 작게.
const INCR_CHUNK: usize = 64 * 1024;
/// 이보다 크면 INCR로 보낸다 — 최대 요청 크기와 무관하게 보수적으로.
const INCR_THRESHOLD: usize = 256 * 1024;
/// 정체된 INCR 송신 수거 시한 — 요청자가 사라졌다.
const XFER_STALE: Duration = Duration::from_secs(10);

// 자주 쓰는 atom 한 벌(연결마다 한 번 인턴). NCLIP_SEL = 읽기 수신 프로퍼티 ·
// NCLIP_TS = TIMESTAMP 트릭 · NCLIP_WAKE = 게시 깨우기 ClientMessage 타입.
x11rb::atom_manager! {
    Atoms:
    AtomsCookie {
        CLIPBOARD,
        TARGETS,
        TIMESTAMP,
        MULTIPLE,
        INCR,
        UTF8_STRING,
        TEXT,
        ATOM_PAIR,
        NCLIP_SEL,
        NCLIP_TS,
        NCLIP_WAKE,
        text_plain: b"text/plain",
        text_plain_utf8: b"text/plain;charset=utf-8",
    }
}

/// `NEXA_CLIP_DIAG=1`일 때만 stderr 진단(watch_linux와 동일 규약).
fn diag(msg: &str) {
    if std::env::var_os("NEXA_CLIP_DIAG").is_some() {
        eprintln!("[diag/x11rb] {msg}");
    }
}

/// 숨은 1×1 창 — 셀렉션 왕복의 우편함. PROPERTY_CHANGE는 INCR·TIMESTAMP 트릭에 필요.
fn hidden_window(conn: &RustConnection, screen_n: usize) -> Result<Window, String> {
    let root = conn
        .setup()
        .roots
        .get(screen_n)
        .ok_or("X 화면 정보 없음")?
        .root;
    let win = conn.generate_id().map_err(|e| format!("XID 할당 실패: {e}"))?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        root,
        -1,
        -1,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(|e| format!("창 생성 실패: {e}"))?;
    Ok(win)
}

// ───────────────────────────── 읽기 (요청자)

struct Reader {
    conn: RustConnection,
    win: Window,
    atoms: Atoms,
}

impl Reader {
    fn connect() -> Result<Self, String> {
        let (conn, screen_n) =
            RustConnection::connect(None).map_err(|e| format!("X 연결 실패: {e}"))?;
        let win = hidden_window(&conn, screen_n)?;
        let atoms = Atoms::new(&conn)
            .map_err(|e| format!("atom 인턴 실패: {e}"))?
            .reply()
            .map_err(|e| format!("atom 인턴 실패: {e}"))?;
        conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
        Ok(Self { conn, win, atoms })
    }

    /// 다음 이벤트 — 마감까지 1ms 간격 폴링(트랜잭션 동안만 돈다 · 유휴 비용 0).
    fn pump(&self, deadline: Instant) -> Option<Event> {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(e)) => return Some(e),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return None,
            }
        }
    }

    fn intern(&self, name: &str) -> Option<Atom> {
        Some(
            self.conn
                .intern_atom(false, name.as_bytes())
                .ok()?
                .reply()
                .ok()?
                .atom,
        )
    }

    /// 타깃 하나를 변환해 받는다 — 소유자 거절(property=None)·시한 초과·상한 초과는 `None`.
    fn convert(&self, target: Atom, cap: usize) -> Option<Vec<u8>> {
        // 직전 트랜잭션 잔재 제거.
        let _ = self.conn.delete_property(self.win, self.atoms.NCLIP_SEL);
        self.conn
            .convert_selection(
                self.win,
                self.atoms.CLIPBOARD,
                target,
                self.atoms.NCLIP_SEL,
                x11rb::CURRENT_TIME,
            )
            .ok()?;
        self.conn.flush().ok()?;
        let deadline = Instant::now() + READ_TIMEOUT;
        loop {
            match self.pump(deadline)? {
                Event::SelectionNotify(n)
                    if n.requestor == self.win && n.selection == self.atoms.CLIPBOARD =>
                {
                    if n.property == x11rb::NONE {
                        return None; // 소유자가 이 타깃을 거절했다.
                    }
                    return self.read_property(cap, deadline);
                }
                _ => {}
            }
        }
    }

    /// 수신 프로퍼티를 읽는다 — type=INCR이면 청크 조립. ⚠️ 상한은 **도중** 집행.
    fn read_property(&self, cap: usize, deadline: Instant) -> Option<Vec<u8>> {
        let (ty, data) = self.fetch_all(cap)?;
        if ty != self.atoms.INCR {
            return (data.len() <= cap).then_some(data);
        }
        // INCR 개시 — fetch_all의 delete가 "받을 준비 됐다" 신호다. 이후 소유자가
        // NewValue로 청크를 쓰고, 우리가 읽어 지우면(delete) 다음 청크가 온다.
        let mut out = Vec::new();
        loop {
            loop {
                match self.pump(deadline)? {
                    Event::PropertyNotify(p)
                        if p.window == self.win
                            && p.atom == self.atoms.NCLIP_SEL
                            && p.state == Property::NEW_VALUE =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
            let (_, chunk) = self.fetch_all(cap)?;
            if chunk.is_empty() {
                return Some(out); // 빈 청크 = 전송 끝.
            }
            out.extend_from_slice(&chunk);
            if out.len() > cap {
                diag(&format!("INCR 수신 상한 {cap}B 초과 — 중단(이름만 담긴다)"));
                return None;
            }
        }
    }

    /// 수신 프로퍼티 전량 읽기(+삭제) — (type, bytes). 상한을 크게 넘기면 중단.
    fn fetch_all(&self, cap: usize) -> Option<(Atom, Vec<u8>)> {
        let mut out = Vec::new();
        let mut ty: Option<Atom> = None;
        let mut off: u32 = 0;
        loop {
            let r = self
                .conn
                .get_property(
                    true,
                    self.win,
                    self.atoms.NCLIP_SEL,
                    AtomEnum::ANY,
                    off,
                    0x0004_0000, // 1MB/회(4바이트 단위).
                )
                .ok()?
                .reply()
                .ok()?;
            if ty.is_none() {
                ty = Some(r.type_);
            }
            out.extend_from_slice(&r.value);
            if r.bytes_after == 0 {
                return Some((ty.unwrap_or(x11rb::NONE), out));
            }
            if out.len() > cap {
                // 남은 것은 관심 없다 — 프로퍼티를 지워 서버 메모리만 돌려준다.
                let _ = self.conn.delete_property(self.win, self.atoms.NCLIP_SEL);
                let _ = self.conn.flush();
                return Some((ty.unwrap_or(x11rb::NONE), out));
            }
            off = off.checked_add(u32::try_from(r.value.len() / 4).ok()?)?;
        }
    }

    /// TARGETS — atom 목록을 이름으로 푼다.
    fn targets(&self, cap: usize) -> Option<Vec<String>> {
        let raw = self.convert(self.atoms.TARGETS, cap)?;
        let mut cookies = Vec::new();
        for chunk in raw.chunks_exact(4) {
            let atom = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if atom != x11rb::NONE {
                cookies.push(self.conn.get_atom_name(atom).ok()?);
            }
        }
        let mut names = Vec::with_capacity(cookies.len());
        for c in cookies {
            names.push(String::from_utf8_lossy(&c.reply().ok()?.name).into_owned());
        }
        Some(names)
    }
}

static READER: OnceLock<Option<Mutex<Reader>>> = OnceLock::new();

fn reader() -> Option<&'static Mutex<Reader>> {
    READER
        .get_or_init(|| match Reader::connect() {
            Ok(r) => Some(Mutex::new(r)),
            Err(e) => {
                diag(&format!("읽기 연결 불가 — {e}"));
                None
            }
        })
        .as_ref()
}

/// 이 환경에서 직접 구현을 쓸 수 있는가 — `DISPLAY` + 실제 연결 성공(1회 캐시).
pub(crate) fn available() -> bool {
    std::env::var_os("DISPLAY").is_some() && reader().is_some()
}

/// 지금 클립보드의 타깃 목록.
pub(crate) fn list_targets(cap: usize) -> Option<Vec<String>> {
    reader()?.lock().ok()?.targets(cap)
}

/// 타깃 하나의 날바이트 — `cap`까지만.
pub(crate) fn read_target(target: &str, cap: usize) -> Option<Vec<u8>> {
    let r = reader()?.lock().ok()?;
    let atom = r.intern(target)?;
    r.convert(atom, cap)
}

// ───────────────────────────── 감시

/// CLIPBOARD 소유자 변경 감시 — 변화마다 `on_change`. ★ 자기 게시(서빙 창)는 원천 차단.
pub(crate) fn watch(on_change: Box<dyn Fn() + Send>) -> Result<(), String> {
    let (conn, screen_n) =
        RustConnection::connect(None).map_err(|e| format!("X 연결 실패: {e}"))?;
    conn.xfixes_query_version(5, 0)
        .map_err(|e| format!("XFIXES 질의 실패: {e}"))?
        .reply()
        .map_err(|e| format!("XFIXES 확장 없음: {e}"))?;
    let win = hidden_window(&conn, screen_n)?;
    let clipboard = conn
        .intern_atom(false, b"CLIPBOARD")
        .map_err(|e| format!("atom 인턴 실패: {e}"))?
        .reply()
        .map_err(|e| format!("atom 인턴 실패: {e}"))?
        .atom;
    conn.xfixes_select_selection_input(
        win,
        clipboard,
        xfixes::SelectionEventMask::SET_SELECTION_OWNER
            | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
            | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE,
    )
    .map_err(|e| format!("셀렉션 감시 등록 실패: {e}"))?;
    conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
    std::thread::Builder::new()
        .name("nclip-x11-watch".into())
        .spawn(move || loop {
            match conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(e)) if e.selection == clipboard => {
                    if owner_window() == Some(e.owner) {
                        diag("자기 게시 에코 — 소유자 창 일치, 건너뜀");
                        continue;
                    }
                    on_change();
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("클립보드 감시(x11rb) 연결 종료: {e}");
                    return;
                }
            }
        })
        .map_err(|e| format!("감시 스레드 생성 실패: {e}"))?;
    Ok(())
}

// ───────────────────────────── 쓰기 (소유자 서빙)

/// 지금 서빙 창의 XID — 감시의 에코 판별용. 0 = 소유 안 함.
static OWNER_WIN: AtomicU32 = AtomicU32::new(0);

fn owner_window() -> Option<Window> {
    let w = OWNER_WIN.load(Ordering::Relaxed);
    (w != 0).then_some(w)
}

/// 게시 한 벌 — (타깃 atom, 응답 type atom, 바이트).
type Entry = (Atom, Atom, Arc<Vec<u8>>);

struct Shared {
    /// 게시 대기 표현들 — 셸 스레드가 넣고 ClientMessage로 깨운다.
    pending: Mutex<Option<Vec<RawRep>>>,
    /// 게시 결과 회신.
    ack: Mutex<Option<Result<(), String>>>,
    cv: Condvar,
}

struct Server {
    conn: Arc<RustConnection>,
    win: Window,
    wake: Atom,
    shared: Arc<Shared>,
}

static SERVER: OnceLock<Result<Server, String>> = OnceLock::new();

fn server() -> Result<&'static Server, String> {
    SERVER
        .get_or_init(|| {
            let (conn, screen_n) =
                RustConnection::connect(None).map_err(|e| format!("X 연결 실패: {e}"))?;
            let conn = Arc::new(conn);
            let win = hidden_window(&conn, screen_n)?;
            let atoms = Atoms::new(conn.as_ref())
                .map_err(|e| format!("atom 인턴 실패: {e}"))?
                .reply()
                .map_err(|e| format!("atom 인턴 실패: {e}"))?;
            conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
            let shared = Arc::new(Shared {
                pending: Mutex::new(None),
                ack: Mutex::new(None),
                cv: Condvar::new(),
            });
            {
                let conn = Arc::clone(&conn);
                let shared = Arc::clone(&shared);
                std::thread::Builder::new()
                    .name("nclip-x11-serve".into())
                    .spawn(move || serve_loop(&conn, win, &atoms, &shared))
                    .map_err(|e| format!("서빙 스레드 생성 실패: {e}"))?;
            }
            Ok(Server {
                conn,
                win,
                wake: atoms.NCLIP_WAKE,
                shared,
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// 표현 **전부**를 게시한다(빈 바이트 제외) — 1단의 "표현 1개" 제약 해소.
/// 반환 = 게시한 표현 수. 서빙 스레드가 소유권을 잡을 때까지 기다린다(ACK_TIMEOUT).
pub(crate) fn set_reps(reps: &[RawRep]) -> Result<usize, String> {
    let posted: Vec<RawRep> = reps.iter().filter(|r| !r.data.is_empty()).cloned().collect();
    if posted.is_empty() {
        return Err("게시할 표현이 없습니다".into());
    }
    let n = posted.len();
    let srv = server()?;
    {
        let mut p = srv.shared.pending.lock().map_err(|_| "락 오염")?;
        *p = Some(posted);
        let mut a = srv.shared.ack.lock().map_err(|_| "락 오염")?;
        *a = None;
    }
    // 서빙 스레드 깨우기 — 자기 창으로 ClientMessage(연결은 Sync라 여기서 보내도 된다).
    let ev = ClientMessageEvent {
        response_type: CLIENT_MESSAGE_EVENT,
        format: 32,
        sequence: 0,
        window: srv.win,
        type_: srv.wake,
        data: [0u32; 5].into(),
    };
    srv.conn
        .send_event(false, srv.win, EventMask::NO_EVENT, ev)
        .map_err(|e| format!("깨우기 실패: {e}"))?;
    srv.conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
    // 회신 대기.
    let mut a = srv.shared.ack.lock().map_err(|_| "락 오염")?;
    let deadline = Instant::now() + ACK_TIMEOUT;
    while a.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("게시 회신 시한 초과".into());
        }
        let (guard, _) = srv
            .shared
            .cv
            .wait_timeout(a, left)
            .map_err(|_| "락 오염")?;
        a = guard;
    }
    match a.take() {
        Some(Ok(())) => Ok(n),
        Some(Err(e)) => Err(e),
        None => Err("게시 회신 유실".into()),
    }
}

/// 진행 중 INCR 송신 하나.
struct Xfer {
    data: Arc<Vec<u8>>,
    ty: Atom,
    off: usize,
    at: Instant,
}

/// 서빙 본체 — 게시 깨우기 · SelectionRequest 응대 · INCR 송신 · 소유권 상실.
fn serve_loop(conn: &Arc<RustConnection>, win: Window, atoms: &Atoms, shared: &Arc<Shared>) {
    let mut current: Vec<Entry> = Vec::new();
    let mut own_time: u32 = x11rb::CURRENT_TIME;
    let mut xfers: HashMap<(Window, Atom), Xfer> = HashMap::new();
    let mut backlog: VecDeque<Event> = VecDeque::new();
    loop {
        let ev = if let Some(e) = backlog.pop_front() {
            e
        } else {
            match conn.wait_for_event() {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("클립보드 서빙(x11rb) 연결 종료: {e}");
                    OWNER_WIN.store(0, Ordering::Relaxed);
                    return;
                }
            }
        };
        // 정체 전송 수거 — 요청자가 사라졌다.
        xfers.retain(|_, x| x.at.elapsed() < XFER_STALE);
        match ev {
            Event::ClientMessage(m) if m.window == win && m.type_ == atoms.NCLIP_WAKE => {
                let reps = shared.pending.lock().ok().and_then(|mut p| p.take());
                if let Some(reps) = reps {
                    let res =
                        publish(conn, win, atoms, &reps, &mut current, &mut own_time, &mut backlog);
                    if res.is_ok() {
                        OWNER_WIN.store(win, Ordering::Relaxed);
                    }
                    if let Ok(mut a) = shared.ack.lock() {
                        *a = Some(res);
                        shared.cv.notify_all();
                    }
                }
            }
            Event::SelectionRequest(r) if r.owner == win => {
                handle_request(conn, atoms, &current, own_time, &r, &mut xfers);
            }
            Event::SelectionClear(c) if c.selection == atoms.CLIPBOARD && c.owner == win => {
                diag("소유권 상실 — 다른 앱이 복사했다");
                current.clear();
                OWNER_WIN.store(0, Ordering::Relaxed);
            }
            Event::PropertyNotify(p) if p.state == Property::DELETE => {
                incr_step(conn, &mut xfers, p.window, p.atom);
            }
            _ => {}
        }
    }
}

/// 게시 — TIMESTAMP 취득(zero-append 트릭 · ICCCM: CurrentTime 금지) 후 소유권 획득·검증.
fn publish(
    conn: &RustConnection,
    win: Window,
    atoms: &Atoms,
    reps: &[RawRep],
    current: &mut Vec<Entry>,
    own_time: &mut u32,
    backlog: &mut VecDeque<Event>,
) -> Result<(), String> {
    conn.change_property8(PropMode::APPEND, win, atoms.NCLIP_TS, AtomEnum::STRING, &[])
        .map_err(|e| format!("TS 트릭 실패: {e}"))?;
    conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
    let deadline = Instant::now() + READ_TIMEOUT;
    let time = loop {
        if Instant::now() >= deadline {
            return Err("서버 시각 취득 시한 초과".into());
        }
        match conn.poll_for_event() {
            Ok(Some(Event::PropertyNotify(p))) if p.window == win && p.atom == atoms.NCLIP_TS => {
                break p.time;
            }
            // 다른 이벤트(SelectionRequest 등)는 잃지 않는다 — 본 루프가 이어서 처리.
            Ok(Some(e)) => backlog.push_back(e),
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => return Err(format!("이벤트 대기 실패: {e}")),
        }
    };
    // 표현 → 서빙 항목. text/plain은 X11 텍스트 atom 별칭까지 광고한다(GTK·Qt 호환).
    let mut entries: Vec<Entry> = Vec::new();
    for r in reps {
        let bytes = Arc::new(r.data.clone());
        let atom = if r.format == "text/plain" {
            atoms.text_plain
        } else {
            conn.intern_atom(false, r.format.as_bytes())
                .map_err(|e| format!("atom 인턴 실패: {e}"))?
                .reply()
                .map_err(|e| format!("atom 인턴 실패: {e}"))?
                .atom
        };
        entries.push((atom, atom, Arc::clone(&bytes)));
        if r.format == "text/plain" {
            let utf8 = atoms.UTF8_STRING;
            entries.push((utf8, utf8, Arc::clone(&bytes)));
            entries.push((Atom::from(AtomEnum::STRING), Atom::from(AtomEnum::STRING), Arc::clone(&bytes)));
            entries.push((atoms.TEXT, utf8, Arc::clone(&bytes)));
            entries.push((atoms.text_plain_utf8, atoms.text_plain_utf8, bytes));
        }
    }
    conn.set_selection_owner(win, atoms.CLIPBOARD, time)
        .map_err(|e| format!("소유권 요청 실패: {e}"))?;
    conn.flush().map_err(|e| format!("flush 실패: {e}"))?;
    let owner = conn
        .get_selection_owner(atoms.CLIPBOARD)
        .map_err(|e| format!("소유권 확인 실패: {e}"))?
        .reply()
        .map_err(|e| format!("소유권 확인 실패: {e}"))?
        .owner;
    if owner != win {
        return Err("소유권 획득 실패 — 다른 앱이 선점".into());
    }
    *current = entries;
    *own_time = time;
    Ok(())
}

/// SelectionRequest 하나 응대 — 못 주는 것은 property=None으로 **정직하게 거절**.
fn handle_request(
    conn: &RustConnection,
    atoms: &Atoms,
    current: &[Entry],
    own_time: u32,
    r: &SelectionRequestEvent,
    xfers: &mut HashMap<(Window, Atom), Xfer>,
) {
    // ICCCM: property가 None인 구식 요청자는 target을 property로 쓴다.
    let prop = if r.property == x11rb::NONE {
        r.target
    } else {
        r.property
    };
    let ok = if r.target == atoms.TARGETS {
        let mut list: Vec<u32> = vec![atoms.TARGETS, atoms.TIMESTAMP, atoms.MULTIPLE];
        list.extend(current.iter().map(|(t, _, _)| *t));
        conn.change_property32(PropMode::REPLACE, r.requestor, prop, AtomEnum::ATOM, &list)
            .is_ok()
    } else if r.target == atoms.TIMESTAMP {
        conn.change_property32(PropMode::REPLACE, r.requestor, prop, AtomEnum::INTEGER, &[own_time])
            .is_ok()
    } else if r.target == atoms.MULTIPLE {
        handle_multiple(conn, atoms, current, r, prop)
    } else {
        serve_value(conn, atoms, current, r.requestor, prop, r.target, true, xfers)
    };
    let reply = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: r.time,
        requestor: r.requestor,
        selection: r.selection,
        target: r.target,
        property: if ok { prop } else { x11rb::NONE },
    };
    let _ = conn.send_event(false, r.requestor, EventMask::NO_EVENT, reply);
    let _ = conn.flush();
}

/// 값 타깃 하나를 프로퍼티에 쓴다 — 크면 INCR 개시(`allow_incr`일 때만).
#[allow(clippy::too_many_arguments)]
fn serve_value(
    conn: &RustConnection,
    atoms: &Atoms,
    current: &[Entry],
    requestor: Window,
    prop: Atom,
    target: Atom,
    allow_incr: bool,
    xfers: &mut HashMap<(Window, Atom), Xfer>,
) -> bool {
    let Some((_, ty, data)) = current.iter().find(|(t, _, _)| *t == target) else {
        return false;
    };
    if data.len() <= INCR_THRESHOLD {
        return conn
            .change_property8(PropMode::REPLACE, requestor, prop, *ty, data)
            .is_ok();
    }
    if !allow_incr {
        return false;
    }
    // INCR 개시 — 요청자의 프로퍼티 삭제(PropertyNotify)를 구독해야 청크를 이어 보낼 수 있다.
    let sub = conn
        .change_window_attributes(
            requestor,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .is_ok();
    let announced = u32::try_from(data.len()).unwrap_or(u32::MAX);
    let ok = sub
        && conn
            .change_property32(PropMode::REPLACE, requestor, prop, atoms.INCR, &[announced])
            .is_ok();
    if ok {
        xfers.insert(
            (requestor, prop),
            Xfer {
                data: Arc::clone(data),
                ty: *ty,
                off: 0,
                at: Instant::now(),
            },
        );
    }
    ok
}

/// MULTIPLE — ATOM_PAIR 목록의 항목별 응대(소형만 · INCR 필요 항목은 None으로 표기).
fn handle_multiple(
    conn: &RustConnection,
    atoms: &Atoms,
    current: &[Entry],
    r: &SelectionRequestEvent,
    prop: Atom,
) -> bool {
    if r.property == x11rb::NONE {
        return false; // MULTIPLE은 property 필수(ICCCM).
    }
    let Ok(Ok(reply)) = conn
        .get_property(false, r.requestor, prop, AtomEnum::ANY, 0, 0x0001_0000)
        .map(|c| c.reply())
    else {
        return false;
    };
    let mut pairs: Vec<u32> = reply
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    let mut dummy: HashMap<(Window, Atom), Xfer> = HashMap::new();
    for pair in pairs.chunks_mut(2) {
        let [t, p] = pair else { continue };
        let served =
            *p != x11rb::NONE && serve_value(conn, atoms, current, r.requestor, *p, *t, false, &mut dummy);
        if !served {
            *t = x11rb::NONE; // 이 항목은 못 준다.
        }
    }
    conn.change_property32(PropMode::REPLACE, r.requestor, prop, atoms.ATOM_PAIR, &pairs)
        .is_ok()
}

/// INCR 다음 청크 — 요청자가 프로퍼티를 지웠다(= 이전 청크를 소화했다).
fn incr_step(
    conn: &RustConnection,
    xfers: &mut HashMap<(Window, Atom), Xfer>,
    win: Window,
    prop: Atom,
) {
    let Some(x) = xfers.get_mut(&(win, prop)) else {
        return;
    };
    let rem = x.data.len() - x.off;
    let n = rem.min(INCR_CHUNK);
    let ok = conn
        .change_property8(PropMode::REPLACE, win, prop, x.ty, &x.data[x.off..x.off + n])
        .is_ok();
    let _ = conn.flush();
    if !ok {
        xfers.remove(&(win, prop));
        return;
    }
    x.off += n;
    x.at = Instant::now();
    if n == 0 {
        xfers.remove(&(win, prop)); // 빈 청크 = 끝을 보냈다.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ 자동 왕복(Xvfb) — 소형 + INCR(1.5MB) 게시를 **자기 읽기 연결**로 회수한다.
    /// `Xvfb :99 & DISPLAY=:99 cargo test -p nclip-plat -- --ignored x11_native`
    #[test]
    #[ignore = "X 서버가 필요(Xvfb 수동 실행 전용)"]
    fn x11_native_roundtrip() {
        let big = vec![0xA5u8; 1_500_000];
        let reps = [
            RawRep {
                format: "text/plain".into(),
                data: b"nexa-clip x11rb".to_vec(),
            },
            RawRep {
                format: "image/png".into(),
                data: big.clone(),
            },
        ];
        let n = set_reps(&reps).expect("게시");
        assert!(n >= 2);
        let targets = list_targets(64 * 1024).expect("TARGETS");
        assert!(targets.iter().any(|t| t == "UTF8_STRING"), "{targets:?}");
        assert!(targets.iter().any(|t| t == "image/png"), "{targets:?}");
        let text = read_target("UTF8_STRING", 1024).expect("텍스트");
        assert_eq!(text, b"nexa-clip x11rb");
        let img = read_target("image/png", 4 * 1024 * 1024).expect("INCR 이미지");
        assert_eq!(img.len(), big.len());
        assert_eq!(img, big);
        // 상한 도중 집행 — 큰 표현을 작은 cap으로 읽으면 정직하게 None.
        assert!(read_target("image/png", 1024).is_none());
    }
}
