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
    if handle.is_empty() || pass.is_empty() {
        eprintln!("동기화: 핸들·페어링 암호가 비어 있습니다 — 설정 → 동기화에서 채운 뒤 재시작");
        return;
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
        .spawn(move || run(&relay_raw, &handle, &pass, &dir, &proxy))
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
        let _ = proxy.send_event(crate::tray_cmd::ShellEvent::SyncState(on));
    };
    // ① 기기 신원(NCK1) — 없으면 생성(포터블: data/ 아래).
    let key_path = dir.join("identity.key");
    let (id, fresh) = match nclip_sync::keyfile::load_or_generate(&key_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("동기화: 신원 키 실패({e}) — 중단");
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
        return;
    };
    let pin_path = dir.join("server.pin");

    // ③ 등록할 RID — 기기 3(에폭 오차) + ★ 페어링 3(같은 핸들·암호 기기가 만나는 지점).
    let mut stage = 0usize;
    loop {
        let mut rids: Vec<nclip_sync::relay::Rid> =
            nclip_sync::relay::rids_around(&id.peer_id()).to_vec();
        rids.extend_from_slice(&nclip_sync::rid::rids_around(handle, pass));

        let expected = nclip_sync::relay::pinfile::lookup(&pin_path, &addr_str);
        match nclip_sync::relay::RelayClient::connect(addr, &id, &rids, expected) {
            Ok(client) => {
                stage = 0;
                notify(true);
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
                    if !client.is_alive() {
                        eprintln!("동기화: 릴레이 세션 끊김 — 재접속");
                        notify(false);
                        break;
                    }
                }
            }
            Err(e) => {
                notify(false);
                eprintln!(
                    "동기화: 접속 실패({e:?}) — {}s 뒤 재시도",
                    BACKOFF_MS[stage] / 1000
                );
            }
        }
        std::thread::sleep(Duration::from_millis(BACKOFF_MS[stage]));
        stage = (stage + 1).min(BACKOFF_MS.len() - 1);
    }
}
