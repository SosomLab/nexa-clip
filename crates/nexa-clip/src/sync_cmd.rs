//! ★ M2 동기화 러너(09-03 기반 3/3) — 신원 키 + 릴레이 접속 상주 스레드.
//!
//! 1단 범위: **접속 기반**까지 — 기기 신원(NCK1) 생성/로드 · 서버 핀(TOFU) ·
//! 기기 RID(3) + ★ 페어링 RID(3 · 핸들+패스프레이즈 파생 — docs/09 §6) 동시 등록 ·
//! 하트비트/재접속 백오프 · 인바운드 랑데부 감지 로그.
//! 종단 승인(DeviceList)·클립보드 전파는 다음 단(docs/07 §2-2 · ADR-0007 v2).

use std::time::Duration;

/// 재접속 백오프(ms) — beep `RECONNECT_BACKOFF_MS` 관례 승계.
const BACKOFF_MS: [u64; 4] = [5_000, 15_000, 60_000, 300_000];
/// 기본 릴레이 — beep 공식 서버(같은 서버 공유 · DP-1 · 포트 기본 47300).
const DEFAULT_RELAY: &str = "beepd.sosomlab.com";

/// ★ 연결 해제 요청(09-03 — 설정 창 Disconnect) · 현재 연결 여부(표시·판정용).
static STOP_REQ: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// 러너 스레드 생존 여부 — 재기동(Test 성공) 때 이중 접속을 막는다(09-03).
static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// ★ 러너 상태(09-03 사용자 — "실행 시 자동으로 Test가 눌린 것처럼"): 설정 창이 폴링해
///   Test 행 노트를 자동 갱신한다. 동기화 꺼짐 = 러너 없음 = `Off`(노트 없음).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SyncStatus {
    Off,
    Connecting,
    Connected,
    Failed(String),
    Stopped,
}
static STATUS: std::sync::Mutex<SyncStatus> = std::sync::Mutex::new(SyncStatus::Off);

fn set_status(st: SyncStatus) {
    if let Ok(mut g) = STATUS.lock() {
        *g = st;
    }
}

/// 현재 러너 상태.
pub(crate) fn status() -> SyncStatus {
    STATUS.lock().map(|g| g.clone()).unwrap_or(SyncStatus::Off)
}

/// ★ 기기 표시 이름 원시값(설정 `sync.device_name` — 비면 호스트명 정제/지문 라벨).
static DEVICE_NAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// 설정에서 이름이 바뀔 때(다음 세션부터 반영 — 기존 세션은 재접속 때).
pub(crate) fn set_device_name(raw: &str) {
    if let Ok(mut g) = DEVICE_NAME.lock() {
        *g = raw.to_string();
    }
}

/// 지금 쓸 표시 이름 — 설정값 무해화, 실패/빈 값이면 호스트명 정제 → `clip-{지문4}`.
pub(crate) fn display_name(me: &nclip_sync::PeerId) -> nclip_sync::name::DisplayName {
    let raw = DEVICE_NAME.lock().map(|g| g.clone()).unwrap_or_default();
    if let Ok(n) = nclip_sync::name::DisplayName::parse(&raw) {
        return n;
    }
    let base = nclip_sync::name::default_display_name(nclip_plat::host::hostname().as_deref(), me);
    // ★ 프로필 실행(09-04) — 같은 PC의 두 인스턴스가 같은 이름으로 보이지 않게 접미를 붙인다.
    match crate::conf::profile() {
        Some(p) => nclip_sync::name::DisplayName::parse(&format!("{base}-{p}")).unwrap_or(base),
        None => base,
    }
}

/// 이 기기의 PeerId 16진(설정 창 "이 기기" 표시) — 러너가 신원을 읽은 뒤 채운다.
static MY_HEX: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// 이 기기의 PeerId 16진(아직 모르면 빈 문자열).
pub(crate) fn my_hex() -> String {
    MY_HEX.lock().map(|g| g.clone()).unwrap_or_default()
}

/// 이 기기의 표시 이름(신원을 아직 모르면 빈 문자열).
pub(crate) fn my_display_name() -> String {
    nclip_sync::relay::parse_peer_hex(&my_hex())
        .map(|p| display_name(&p).to_string())
        .unwrap_or_default()
}

/// 피어 세션 세대 — 러너가 끊기거나 재접속하면 올린다(옛 세션 스레드가 스스로 물러난다).
static PEER_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// 동시 다이얼 상한(스레드 폭주 방지).
static DIALING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// 알려진 기기 재다이얼·페어링 탐색 **기본** 주기 — 실패마다 2배(상한 아래) · 성공/상대 등장 시 초기화.
const DIAL_EVERY: Duration = Duration::from_secs(10);
/// 기기별 재다이얼 백오프 상한(5분) · 페어링 탐색 백오프 상한(2분).
const DIAL_MAX: Duration = Duration::from_secs(300);
const PAIR_MAX: Duration = Duration::from_secs(120);
/// 인바운드 랑데부 동시 핸드셰이크 상한(폭주 시 나머지는 버림 — 상대가 다시 연다).
const INBOUND_MAX: usize = 4;
/// 피어별 수신 예산 — 10초 창에 24MB(항목 상한 32MB와 별개 · 홍수 차단).
const RX_WINDOW: Duration = Duration::from_secs(10);
const RX_BUDGET: usize = 24 * 1024 * 1024;
/// 같은 페이로드 재전송 억제 창(승격 에코·연타).
const DEDUPE_WINDOW: Duration = Duration::from_secs(2);
static INBOUND_HS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static LAST_SENT: std::sync::Mutex<Option<(u64, std::time::Instant)>> = std::sync::Mutex::new(None);
/// 세션 핑 주기 · 무응답 한계.
const PEER_PING: Duration = Duration::from_secs(15);
const PEER_DEAD: Duration = Duration::from_secs(45);

/// ★ 피어별 송신 채널(09-04) — 세션 스레드가 등록·해제, `broadcast`가 밀어 넣는다.
type PeerTx = std::sync::mpsc::Sender<std::sync::Arc<Vec<u8>>>;
static PEER_TX: std::sync::Mutex<Vec<(String, PeerTx)>> = std::sync::Mutex::new(Vec::new());
static ITEM_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// 보낼 수 있는 기기(승인 + 온라인)가 하나라도 있는가 — UI 스레드가 워커를 띄울지 판정(값싼 검사).
pub(crate) fn has_peers() -> bool {
    PEER_TX.lock().is_ok_and(|g| {
        g.iter()
            .any(|(hex, _)| crate::devices::is_approved(hex) && crate::devices::is_online(hex))
    })
}

/// ★ 클립보드 항목 전파(DR-6) — **승인된** 온라인 기기 전부에 보낸다. 반환 = 보낸 기기 수.
pub(crate) fn broadcast(payload: Vec<u8>) -> usize {
    // ★ 같은 내용 연타 억제(09-04 과다 트래픽 예방) — 승격 에코·앱의 반복 게시가 같은 바이트를 만든다.
    let h = crate::syncitem::hash(&payload);
    if let Ok(mut g) = LAST_SENT.lock() {
        if let Some((lh, t)) = *g {
            if lh == h && t.elapsed() < DEDUPE_WINDOW {
                return 0;
            }
        }
        *g = Some((h, std::time::Instant::now()));
    }
    let arc = std::sync::Arc::new(payload);
    let Ok(g) = PEER_TX.lock() else {
        return 0;
    };
    g.iter()
        .filter(|(hex, _)| crate::devices::is_approved(hex) && crate::devices::is_online(hex))
        .filter(|(_, tx)| tx.send(arc.clone()).is_ok())
        .count()
}

fn wake(proxy: &winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>) {
    let _ = proxy.send_event(crate::tray_cmd::ShellEvent::SyncTick);
}

/// ★ 종단 세션 하나의 수명(09-03) — 인사 교환 → 목록 등재 → 핑/퐁 상주 → 종료 시 오프라인.
///   같은 기기와 세션이 이미 있으면 새 것을 버린다(글레어·중복 다이얼 흡수).
pub(crate) fn run_peer(
    mut s: nclip_sync::NoiseSession<Box<dyn nclip_sync::Link>>,
    me: nclip_sync::PeerId,
    gen_slot: &'static std::sync::atomic::AtomicU64,
    gen: u64,
    proxy: winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>,
    dir: std::path::PathBuf,
    via: &'static str,
) {
    use nclip_sync::hello::{Hello, PeerMsg};
    use nclip_sync::session::{Session as _, SessionError};
    let hex = nclip_sync::relay::peer_hex(&s.peer());
    if hex == nclip_sync::relay::peer_hex(&me) {
        return; // 자기 자신(서버가 막지만 이중 방어)
    }
    if crate::devices::is_online(&hex) {
        println!("동기화: 기기 {}… 세션 중복 — 새 것을 버림", &hex[..8]);
        return;
    }
    let hello = Hello::local(display_name(&me));
    if s.send(&PeerMsg::Hello(hello).encode()).is_err() {
        return;
    }
    // ★ 송신 채널 등록(09-04) — 셸의 broadcast가 여기로 항목을 민다.
    let (tx, rx) = std::sync::mpsc::channel::<std::sync::Arc<Vec<u8>>>();
    if let Ok(mut g) = PEER_TX.lock() {
        g.retain(|(h, _)| h != &hex);
        g.push((hex.clone(), tx));
    }
    s.set_recv_timeout(Some(Duration::from_millis(250)));
    let devices_path = dir.join("devices.txt");
    let mut last_ping = std::time::Instant::now();
    let mut last_rx = std::time::Instant::now();
    let mut greeted = false;
    let mut peer_name = String::new();
    let mut asm = nclip_sync::hello::Assembler::default();
    // ★ 수신 예산(09-04 과다 트래픽 예방) — 창 안에서 예산을 넘긴 조각은 조립하지 않는다.
    let mut rx_win = std::time::Instant::now();
    let mut rx_bytes = 0usize;
    let mut rx_warned = false;
    'session: loop {
        if gen_slot.load(std::sync::atomic::Ordering::Relaxed) != gen {
            break;
        }
        match s.recv() {
            Ok(m) => {
                last_rx = std::time::Instant::now();
                match PeerMsg::decode(&m) {
                    Some(PeerMsg::Item {
                        seq,
                        idx,
                        total,
                        data,
                    }) => {
                        if rx_win.elapsed() > RX_WINDOW {
                            rx_win = std::time::Instant::now();
                            rx_bytes = 0;
                            rx_warned = false;
                        }
                        rx_bytes += data.len();
                        if rx_bytes > RX_BUDGET {
                            if !rx_warned {
                                eprintln!(
                                    "동기화: {}({}…) 수신 예산 초과({}MB/10s) — 이 창의 나머지는 버림",
                                    peer_name,
                                    &hex[..8],
                                    RX_BUDGET / 1_048_576
                                );
                                rx_warned = true;
                            }
                            continue;
                        }
                        if let Some(payload) = asm.push(seq, idx, total, data) {
                            if crate::devices::is_approved(&hex) {
                                // ★ 디코드·OS 표현 변환(PNG 디코드 포함)·에코 지문은 **여기(세션 스레드)**서 —
                                //   UI 스레드는 이력 등재·게시만 한다(09-04 사용자 요구).
                                match crate::syncitem::decode(&payload) {
                                    Some(parts) => {
                                        let reps = crate::syncitem::to_local_reps(&parts);
                                        if reps.is_empty() {
                                            eprintln!("동기화: {peer_name}의 항목에 이 OS로 옮길 표현이 없음 — 버림");
                                        } else {
                                            let skip_hash = crate::syncitem::from_reps(&reps)
                                                .map(|p| crate::syncitem::hash(&p));
                                            let _ = proxy.send_event(
                                                crate::tray_cmd::ShellEvent::SyncItem {
                                                    from: peer_name.clone(),
                                                    summary: crate::syncitem::describe(&parts),
                                                    reps,
                                                    skip_hash,
                                                },
                                            );
                                        }
                                    }
                                    None => {
                                        eprintln!("동기화: {peer_name}의 항목 형식 오류 — 버림")
                                    }
                                }
                            } else {
                                println!(
                                    "동기화: {}({}…)의 클립보드 항목 — 승인 전이라 버림(설정 → 동기화 → 승인)",
                                    peer_name,
                                    &hex[..8]
                                );
                            }
                        }
                    }
                    Some(PeerMsg::Hello(h)) => {
                        peer_name = h.name.as_str().to_string();
                        let new = crate::devices::upsert_online(&hex, h.name.as_str(), &h.os, via);
                        if let Err(e) = crate::devices::save(&devices_path) {
                            eprintln!("동기화: 기기 목록 저장 실패({e})");
                        }
                        println!(
                            "동기화: ★ 기기 연결({via}) — {} ({}… · {}){}",
                            h.name,
                            &hex[..8],
                            h.os,
                            if new { " · 새 기기" } else { "" }
                        );
                        greeted = true;
                        wake(&proxy);
                    }
                    Some(PeerMsg::Ping) => {
                        if s.send(&PeerMsg::Pong.encode()).is_err() {
                            break;
                        }
                    }
                    Some(PeerMsg::Pong) | None => {}
                }
            }
            Err(SessionError::TimedOut) => {}
            Err(_) => break,
        }
        // ★ 송신 — 셸이 민 항목을 조각내 보낸다(승인 판정은 broadcast가 이미 했다).
        // ★ 송신 합치기(09-04) — 밀린 항목은 **가장 최신 하나만** 보낸다(클립보드 의미: 마지막이 곧 현재).
        let mut latest = None;
        let mut skipped = 0usize;
        while let Ok(payload) = rx.try_recv() {
            if latest.is_some() {
                skipped += 1;
            }
            latest = Some(payload);
        }
        if let Some(payload) = latest {
            let seq = ITEM_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            for m in PeerMsg::chunks(seq, &payload) {
                if s.send(&m.encode()).is_err() {
                    break 'session;
                }
            }
            println!(
                "동기화: → {}({}…) 항목 {}KB{}",
                peer_name,
                &hex[..8],
                payload.len() / 1024,
                if skipped > 0 {
                    format!(" (밀린 {skipped}개 건너뜀)")
                } else {
                    String::new()
                }
            );
        }
        if last_ping.elapsed() >= PEER_PING {
            if s.send(&PeerMsg::Ping.encode()).is_err() {
                break;
            }
            last_ping = std::time::Instant::now();
        }
        if last_rx.elapsed() > PEER_DEAD {
            break;
        }
    }
    if let Ok(mut g) = PEER_TX.lock() {
        g.retain(|(h, _)| h != &hex);
    }
    if greeted {
        crate::devices::set_offline(&hex);
        let _ = crate::devices::save(&devices_path);
        println!("동기화: 기기 {}… 세션 종료", &hex[..8]);
        wake(&proxy);
    }
}

/// 다이얼 스레드 하나(상한 안에서) — 성립하면 세션 상주로 이어진다.
fn spawn_dial<F>(label: String, f: F)
where
    F: FnOnce() -> Option<nclip_sync::NoiseSession<Box<dyn nclip_sync::Link>>> + Send + 'static,
    F: FnOnce() -> Option<nclip_sync::NoiseSession<Box<dyn nclip_sync::Link>>>,
{
    if DIALING.load(std::sync::atomic::Ordering::Relaxed) >= 4 {
        return;
    }
    DIALING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let _ = std::thread::Builder::new().name(label).spawn(move || {
        let out = f();
        DIALING.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        drop(out);
    });
}

/// 러너 스레드가 살아 있는가(테스트가 RID 충돌을 피해 기다리는 데 쓴다).
pub(crate) fn is_running() -> bool {
    RUNNING.load(std::sync::atomic::Ordering::Relaxed)
}

/// 러너 생존 표시 해제 가드 — 어떤 return 경로로 나가도 반드시 내린다.
struct RunGuard;
impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 지금 릴레이에 연결돼 있는가.
pub(crate) fn is_connected() -> bool {
    CONNECTED.load(std::sync::atomic::Ordering::Relaxed)
}

/// 마지막 접속 성공 정보(주소, 서버 핀 앞 8자) — 설정 창 노트 시드(09-03).
static LAST_OK: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

/// 마지막 성공 정보 기록(러너·테스트 공용).
pub(crate) fn note_last_ok(addr: &str, pin8: &str) {
    if let Ok(mut g) = LAST_OK.lock() {
        *g = Some((addr.to_string(), pin8.to_string()));
    }
}

/// 마지막 성공 정보 조회.
pub(crate) fn last_ok() -> Option<(String, String)> {
    LAST_OK.lock().ok().and_then(|g| g.clone())
}

/// 마지막 성공 정보 삭제 — 해제(Disconnect·설정 변경) 때 Connected 노트 시드를 지운다.
pub(crate) fn clear_last_ok() {
    if let Ok(mut g) = LAST_OK.lock() {
        *g = None;
    }
}

/// ★ 연결 해제 요청 — 러너가 세션을 끊고 스레드를 마친다(재연결 = 다음 시작).
pub(crate) fn request_disconnect() {
    STOP_REQ.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::lan::bump(); // LAN 직결도 함께 내린다(설정 변경 = 태그 변경).
}

/// `sync.enabled`가 켜져 있으면 접속 스레드를 띄운다(부팅 시 1회 — 동적 반영은 후속).
pub(crate) fn spawn_if_enabled(
    conf: &crate::conf::Settings,
    proxy: winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>,
) {
    if conf.state.get("sync.enabled") != "on" {
        return;
    }
    let handle = conf.state.get("sync.handle").trim().to_string();
    let pass = conf.state.get("sync.passphrase").trim().to_string();
    set_device_name(conf.state.get("sync.device_name"));
    // ★ 핸들/암호 없이도 접속은 한다(09-03 — 연결 상태 유지 계약: Test 성공 시 자동 켬).
    //   페어링 랑데부만 생략되고, 기기 RID 등록·상태 표시는 그대로다.
    if handle.is_empty() || pass.is_empty() {
        println!("동기화: 핸들·페어링 암호 미설정 — 접속만 유지(기기 간 만남은 설정 후)");
    }
    let relay_raw = {
        let r = conf.state.get("sync.relay").trim().to_string();
        let a = if r.is_empty() {
            DEFAULT_RELAY.to_string()
        } else {
            r
        };
        let p = conf.state.get("sync.port").trim().to_string();
        if p.is_empty() || a.contains(':') {
            a
        } else {
            format!("{a}:{p}")
        }
    };
    let dir = crate::conf::data_dir();
    // ★ 신원은 여기서 **한 번** 로드(09-04) — 릴레이 러너와 LAN 직결이 같은 키를 쓴다
    //   (각자 load_or_generate하면 첫 실행에 서로 다른 키를 만들 수 있다).
    let key_path = dir.join("identity.key");
    let id = match nclip_sync::keyfile::load_or_generate(&key_path) {
        Ok((id, fresh)) => {
            let me = nclip_sync::relay::peer_hex(&id.peer_id());
            if let Ok(mut g) = MY_HEX.lock() {
                *g = me.clone();
            }
            println!(
                "동기화: 기기 신원 {}{} — {} · 표시 이름 {}",
                &me[..16],
                if fresh { " (새로 생성)" } else { "" },
                key_path.display(),
                display_name(&id.peer_id())
            );
            std::sync::Arc::new(id)
        }
        Err(e) => {
            eprintln!("동기화: 신원 키 실패({e}) — 중단");
            set_status(SyncStatus::Failed(format!("identity: {e}")));
            wake(&proxy);
            return;
        }
    };
    crate::devices::load(&dir.join("devices.txt"));
    // ★ 같은 네트워크 직결(09-04) — 릴레이와 독립(서버 없이도 만난다).
    crate::lan::spawn(&handle, &pass, id.clone(), proxy.clone(), dir.clone());
    std::thread::Builder::new()
        .name("nclip-sync".into())
        .spawn(move || {
            // ★ 재기동(09-03 — Test 성공이 다시 부른다): 이전 러너가 살아 있으면
            //   끊고 비켜줄 때까지 잠깐 기다린다(1 RID = 1 연결 — 이중 접속 금지).
            if RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                STOP_REQ.store(true, std::sync::atomic::Ordering::Relaxed);
                for _ in 0..50 {
                    if !RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if RUNNING.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("동기화: 이전 세션이 아직 정리 중 — 재접속 포기(다음 시작 때)");
                return;
            }
            let _alive = RunGuard;
            run(&relay_raw, &handle, &pass, &dir, &proxy, id);
        })
        .ok();
}

/// 상주 루프 — 실패는 백오프 후 재시도(상주 앱: 조용히 버티고 로그로 알린다).
fn run(
    relay_raw: &str,
    handle: &str,
    pass: &str,
    dir: &std::path::Path,
    proxy: &winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>,
    id: std::sync::Arc<nclip_sync::Identity>,
) {
    // ★ 연결 상태 통지(09-03) — 트레이 녹색 점·메인창 인디케이터가 이 신호를 그린다.
    let notify = |on: bool| {
        CONNECTED.store(on, std::sync::atomic::Ordering::Relaxed);
        let _ = proxy.send_event(crate::tray_cmd::ShellEvent::SyncState(on));
    };
    // 상태만 바뀐 경우(접속 중·실패·중단) 이벤트 루프를 깨워 설정 창이 폴링하게 한다.
    let tick = || {
        let _ = proxy.send_event(crate::tray_cmd::ShellEvent::SyncTick);
    };
    STOP_REQ.store(false, std::sync::atomic::Ordering::Relaxed);
    // ① 기기 신원은 spawn_if_enabled가 로드해 넘겼다(릴레이·LAN 공용).
    let me = nclip_sync::relay::peer_hex(&id.peer_id());

    // ② 서버 주소·핀 준비.
    let Some((addr_str, addr)) = nclip_sync::relay::resolve_server(relay_raw) else {
        eprintln!("동기화: 릴레이 주소 해석 실패 — {relay_raw}");
        set_status(SyncStatus::Failed(format!("resolve: {relay_raw}")));
        tick();
        return;
    };
    let pin_path = dir.join("server.pin");

    // ③ 등록할 RID — 기기 3(에폭 오차) + ★ 페어링 3(같은 핸들·암호 기기가 만나는 지점).
    let mut stage = 0usize;
    loop {
        set_status(SyncStatus::Connecting);
        tick();
        let mut rids: Vec<nclip_sync::relay::Rid> =
            nclip_sync::relay::rids_around(&id.peer_id()).to_vec();
        if !handle.is_empty() && !pass.is_empty() {
            rids.extend_from_slice(&nclip_sync::rid::rids_around(handle, pass));
        }

        let expected = nclip_sync::relay::pinfile::lookup(&pin_path, &addr_str);
        match nclip_sync::relay::RelayClient::connect(addr, &id, &rids, expected) {
            Ok(client) => {
                stage = 0;
                set_status(SyncStatus::Connected);
                notify(true);
                note_last_ok(
                    &addr_str,
                    &nclip_sync::relay::peer_hex(&client.server_peer())[..8],
                );
                let info = client.register_info();
                println!(
                    "동기화: 릴레이 접속 ok — {addr_str} · 관측 주소 {:?} · UDP 포트 {}",
                    info.observed_tcp, info.udp_port
                );
                if expected.is_none() {
                    if let Err(e) = nclip_sync::relay::pinfile::store(
                        &pin_path,
                        &addr_str,
                        &client.server_peer(),
                    ) {
                        eprintln!("동기화: 서버 핀 저장 실패({e}) — 다음 접속도 TOFU");
                    } else {
                        println!("동기화: 서버 핀 고정(TOFU) — {}", pin_path.display());
                    }
                }
                // ④ 상주 — ★ 인바운드 수락(종단 세션 → 이름 교환) + 알려진 기기 다이얼 +
                //   페어링 랑데부 탐색(09-03). 승인(DeviceList)·클립보드 전파는 다음 단.
                let client = std::sync::Arc::new(client);
                let gen = PEER_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let me_peer = id.peer_id();
                let pair_rids: Vec<nclip_sync::relay::Rid> = if handle.is_empty() || pass.is_empty()
                {
                    Vec::new()
                } else {
                    nclip_sync::rid::rids_around(handle, pass).to_vec()
                };
                let mut last_dial = std::time::Instant::now() - DIAL_EVERY;
                // ★ 백오프 상태(09-04 과다 트래픽 예방) — 기기별 다음 시도 시각·간격, 페어링 탐색 간격.
                let mut dial_next: std::collections::HashMap<
                    String,
                    (std::time::Instant, Duration),
                > = std::collections::HashMap::new();
                let mut pair_wait = DIAL_EVERY;
                let mut pair_next = std::time::Instant::now();
                loop {
                    if let Some(inc) = client.accept_incoming(Duration::from_secs(1)) {
                        if INBOUND_HS.load(std::sync::atomic::Ordering::Relaxed) >= INBOUND_MAX {
                            eprintln!(
                                "동기화: 인바운드 랑데부 폭주 — 동시 {INBOUND_MAX} 초과분은 버림"
                            );
                            drop(inc);
                            continue;
                        }
                        println!(
                            "동기화: 인바운드 랑데부(src RID {:02x}{:02x}…) — 종단 수락 시도",
                            inc.src[0], inc.src[1]
                        );
                        let (c, i, p, d) =
                            (client.clone(), id.clone(), proxy.clone(), dir.to_path_buf());
                        INBOUND_HS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = std::thread::Builder::new()
                            .name("nclip-peer-in".into())
                            .spawn(move || {
                                struct Dec;
                                impl Drop for Dec {
                                    fn drop(&mut self) {
                                        INBOUND_HS
                                            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                                let _dec = Dec;
                                match nclip_sync::relay::accept_via(
                                    &c,
                                    inc,
                                    &i,
                                    true,
                                    Duration::from_secs(10),
                                ) {
                                    Ok(via) => run_peer(
                                        via.session,
                                        me_peer,
                                        &PEER_GEN,
                                        gen,
                                        p,
                                        d,
                                        "relay",
                                    ),
                                    Err(e) => eprintln!("동기화: 인바운드 수락 실패({e:?})"),
                                }
                            });
                    }
                    if last_dial.elapsed() >= DIAL_EVERY {
                        last_dial = std::time::Instant::now();
                        let now = std::time::Instant::now();
                        // 알려진 기기 — 오프라인이고 **내 키가 작은 쪽**만 건다(글레어 회피 타이브레이크).
                        //   ★ 기기별 지수 백오프(10s → … → 5분) — 꺼진 기기에 매 10초 릴레이 Open을 던지지 않는다.
                        for hex in crate::devices::known_hex() {
                            if crate::devices::is_online(&hex) {
                                dial_next.remove(&hex); // 붙었다 = 백오프 초기화
                                continue;
                            }
                            if me >= hex {
                                continue;
                            }
                            let (next, wait) =
                                dial_next.get(&hex).copied().unwrap_or((now, DIAL_EVERY));
                            if now < next {
                                continue;
                            }
                            dial_next.insert(hex.clone(), (now + wait, (wait * 2).min(DIAL_MAX)));
                            let Some(peer) = nclip_sync::relay::parse_peer_hex(&hex) else {
                                continue;
                            };
                            let (c, i, p, d) =
                                (client.clone(), id.clone(), proxy.clone(), dir.to_path_buf());
                            spawn_dial(format!("nclip-peer-dial-{}", &hex[..8]), move || {
                                match nclip_sync::relay::connect_via(
                                    &c,
                                    &i,
                                    &peer,
                                    true,
                                    Duration::from_secs(3),
                                ) {
                                    Ok(via) => run_peer(
                                        via.session,
                                        me_peer,
                                        &PEER_GEN,
                                        gen,
                                        p,
                                        d,
                                        "relay",
                                    ),
                                    Err(nclip_sync::relay::ViaError::NotFound) => {}
                                    Err(e) => {
                                        eprintln!("동기화: 기기 {}… 다이얼 실패({e:?})", &hex[..8])
                                    }
                                }
                                None
                            });
                        }
                        // 페어링 랑데부 — 아직 아무와도 안 붙었을 때만 만남 지점을 연다
                        //   (서버는 최신 등록자에게 잇고, 내가 등록자면 "대상 없음"으로 돌아온다).
                        let any_online = crate::devices::list().iter().any(|d| d.online);
                        if any_online {
                            pair_wait = DIAL_EVERY; // 누군가 붙어 있으면 다음 탐색은 기본 주기부터
                        }
                        // ★ 페어링 탐색 백오프(10s → … → 2분) — 상대가 없을 때 릴레이에 Open을 연타하지 않는다.
                        let pair_due = std::time::Instant::now() >= pair_next;
                        if !pair_rids.is_empty() && !any_online && pair_due {
                            pair_next = std::time::Instant::now() + pair_wait;
                            pair_wait = (pair_wait * 2).min(PAIR_MAX);
                            let (c, i, p, d) =
                                (client.clone(), id.clone(), proxy.clone(), dir.to_path_buf());
                            let rids = pair_rids.clone();
                            spawn_dial("nclip-peer-pair".into(), move || {
                                for rid in rids {
                                    match nclip_sync::relay::connect_rid(
                                        &c,
                                        &i,
                                        rid,
                                        Duration::from_secs(3),
                                    ) {
                                        Ok(session) => {
                                            run_peer(
                                                session, me_peer, &PEER_GEN, gen, p, d, "relay",
                                            );
                                            break;
                                        }
                                        Err(nclip_sync::relay::ViaError::NotFound) => {}
                                        Err(e) => {
                                            eprintln!("동기화: 페어링 랑데부 실패({e:?})");
                                            break;
                                        }
                                    }
                                }
                                None
                            });
                        }
                    }
                    // ★ 사용자 해제(09-03) — 세션을 버리고 스레드를 마친다.
                    if STOP_REQ.swap(false, std::sync::atomic::Ordering::Relaxed) {
                        drop(client);
                        PEER_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        crate::devices::all_offline_via("relay");
                        set_status(SyncStatus::Stopped);
                        notify(false);
                        println!("동기화: 사용자 요청으로 연결 해제 — 다음 시작 때 재연결");
                        return;
                    }
                    if !client.is_alive() {
                        eprintln!("동기화: 릴레이 세션 끊김 — 재접속");
                        PEER_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        crate::devices::all_offline_via("relay");
                        notify(false);
                        break;
                    }
                }
            }
            Err(e) => {
                set_status(SyncStatus::Failed(format!("{e:?}")));
                notify(false);
                eprintln!(
                    "동기화: 접속 실패({e:?}) — {}s 뒤 재시도",
                    BACKOFF_MS[stage] / 1000
                );
            }
        }
        if STOP_REQ.swap(false, std::sync::atomic::Ordering::Relaxed) {
            println!("동기화: 사용자 요청으로 중단");
            set_status(SyncStatus::Stopped);
            tick();
            return;
        }
        std::thread::sleep(Duration::from_millis(BACKOFF_MS[stage]));
        stage = (stage + 1).min(BACKOFF_MS.len() - 1);
    }
}
