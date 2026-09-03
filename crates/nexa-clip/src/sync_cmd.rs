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
            run(&relay_raw, &handle, &pass, &dir, &proxy);
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
    // ① 기기 신원(NCK1) — 없으면 생성(포터블: data/ 아래).
    let key_path = dir.join("identity.key");
    let (id, fresh) = match nclip_sync::keyfile::load_or_generate(&key_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("동기화: 신원 키 실패({e}) — 중단");
            set_status(SyncStatus::Failed(format!("identity: {e}")));
            tick();
            return;
        }
    };
    let me = nclip_sync::relay::peer_hex(&id.peer_id());
    println!(
        "동기화: 기기 신원 {}{} — {}",
        &me[..16],
        if fresh { " (새로 생성)" } else { "" },
        key_path.display()
    );

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
                // ④ 상주 — 인바운드 랑데부 감지(승인·전파는 다음 단).
                loop {
                    if let Some(inc) = client.accept_incoming(Duration::from_secs(1)) {
                        println!(
                            "동기화: ★ 인바운드 랑데부 — 같은 만남 지점의 기기가 접속 시도 \
(src RID {:02x}{:02x}… · 승인 체계는 다음 단이라 아직 연결하지 않습니다)",
                            inc.src[0], inc.src[1]
                        );
                        drop(inc);
                    }
                    // ★ 사용자 해제(09-03) — 세션을 버리고 스레드를 마친다.
                    if STOP_REQ.swap(false, std::sync::atomic::Ordering::Relaxed) {
                        drop(client);
                        set_status(SyncStatus::Stopped);
                        notify(false);
                        println!("동기화: 사용자 요청으로 연결 해제 — 다음 시작 때 재연결");
                        return;
                    }
                    if !client.is_alive() {
                        eprintln!("동기화: 릴레이 세션 끊김 — 재접속");
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
