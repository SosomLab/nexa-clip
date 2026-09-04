//! ★ 같은 네트워크 직결(09-04 사용자 요청 · T-25/T-26 1단 · docs/09 §7 경로 A) — **릴레이 서버 불요**.
//!
//! - 발견: UDP 브로드캐스트 비콘(`47301` · 5초) — `"NCLB"` ‖ ver ‖ **LAN 태그 16B** ‖ 내 PeerId(hex 64) ‖ TCP 포트.
//!   태그 = `sha256("nclip-lan-v1" ‖ 페어링 RID)` — 같은 핸들·암호 기기만 같은 태그를 낸다. 릴레이 RID 자체는
//!   싣지 않는다(LAN 도청자가 만남 지점을 얻지 못하게). 어제·오늘·내일 RID 셋을 다 받아 자정 오차를 흡수한다.
//! - 연결: 태그가 맞고 내 키 hex가 **작은 쪽**이 TCP로 건다(glare 회피 · 릴레이 경로와 같은 규칙) → 같은
//!   Noise 핸드셰이크(prologue `nexa-clip/1`) → 세션이 확정한 키가 비콘의 PeerId와 같아야 한다(비콘은 힌트일 뿐).
//! - 이후는 릴레이 경로와 **같은 세션**(`sync_cmd::run_peer`): 인사·기기 목록·승인·전파. 목록엔 `LAN`으로 표기.
//!
//! 트래픽은 전부 Noise 안(DR-4). 릴레이가 붙어 있어도 LAN 세션이 먼저 서면 그쪽이 쓰인다(중복 세션은 뒤 것을 버림).

use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 비콘 포트(릴레이 47300 다음).
const BEACON_PORT: u16 = 47301;
const BEACON_EVERY: Duration = Duration::from_secs(5);
const MAGIC: &[u8; 4] = b"NCLB";
const DIAL_TIMEOUT: Duration = Duration::from_secs(3);
const HS_TIMEOUT: Duration = Duration::from_secs(10);
/// ★ 과다 트래픽 예방(09-04): LAN 피어가 붙어 있으면 비콘을 느리게(발견은 이미 됐다) · 초당 비콘 수신 상한 ·
/// 동시 인바운드 핸드셰이크 상한 · 실패한 상대 IP 냉각 · 기기별 다이얼 백오프.
const BEACON_SLOW: Duration = Duration::from_secs(30);
const BEACON_RX_PER_SEC: u32 = 20;
const INBOUND_MAX: usize = 4;
const FAIL_COOLDOWN: Duration = Duration::from_secs(30);
static INBOUND_HS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// LAN 세대 — 러너 재기동·해제 때 올리면 비콘·수락·세션 스레드가 스스로 물러난다.
pub(crate) static LAN_GEN: AtomicU64 = AtomicU64::new(0);

/// 세대 올림(해제·재기동).
pub(crate) fn bump() {
    LAN_GEN.fetch_add(1, Ordering::Relaxed);
}

/// LAN 태그 [어제, 오늘, 내일] — 도메인 분리 해시는 nclip-sync 몫.
fn lan_tags(handle: &str, pass: &str) -> Vec<[u8; 16]> {
    nclip_sync::rid::lan_tags(handle, pass).to_vec()
}

fn encode_beacon(tag: &[u8; 16], me_hex: &str, port: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + 1 + 16 + 64 + 2);
    v.extend_from_slice(MAGIC);
    v.push(1);
    v.extend_from_slice(tag);
    v.extend_from_slice(me_hex.as_bytes());
    v.extend_from_slice(&port.to_le_bytes());
    v
}

/// (태그, peer hex, 포트).
fn decode_beacon(b: &[u8]) -> Option<([u8; 16], String, u16)> {
    if b.len() != 4 + 1 + 16 + 64 + 2 || &b[..4] != MAGIC || b[4] != 1 {
        return None;
    }
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&b[5..21]);
    let hex = std::str::from_utf8(&b[21..85]).ok()?;
    if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let port = u16::from_le_bytes([b[85], b[86]]);
    Some((tag, hex.to_string(), port))
}

/// UDP 비콘 소켓 — 같은 PC의 두 인스턴스(`--profile`)도 함께 듣도록 주소 재사용 + 브로드캐스트.
fn beacon_socket() -> std::io::Result<UdpSocket> {
    nclip_sync::tcp::udp_beacon_socket(BEACON_PORT)
}

/// LAN 직결 시작(핸들·암호가 있어야 태그가 선다). 이전 세대는 물러나고 새 세대가 선다.
pub(crate) fn spawn(
    handle: &str,
    pass: &str,
    id: Arc<nclip_sync::Identity>,
    proxy: winit::event_loop::EventLoopProxy<crate::tray_cmd::ShellEvent>,
    dir: std::path::PathBuf,
) {
    if handle.is_empty() || pass.is_empty() {
        println!("LAN 직결: 핸들·페어링 암호가 없어 쉼(설정 후 Test)");
        return;
    }
    let gen = LAN_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let tags = lan_tags(handle, pass);
    let me_hex = nclip_sync::relay::peer_hex(&id.peer_id());
    let me = id.peer_id();

    // ① 직결 수락 — 임의 포트(비콘이 알린다).
    let listener = match TcpListener::bind(("0.0.0.0", 0)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("LAN 직결: 수락 소켓 실패({e}) — LAN 경로 없이 진행");
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let _ = listener.set_nonblocking(true);
    // 실패한 상대 IP → 냉각 만료 시각(핸드셰이크 스레드가 기록 · 수락 스레드가 참조).
    let cooldown: Arc<std::sync::Mutex<std::collections::HashMap<IpAddr, Instant>>> =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    {
        let (id, proxy, dir) = (id.clone(), proxy.clone(), dir.clone());
        let _ = std::thread::Builder::new()
            .name("nclip-lan-accept".into())
            .spawn(move || {
                while LAN_GEN.load(Ordering::Relaxed) == gen {
                    match listener.accept() {
                        Ok((stream, from)) => {
                            // ★ Windows는 논블로킹 리스너에서 받은 스트림이 논블로킹을 **상속**한다
                            //   (Linux는 아님) — 되돌리지 않으면 핸드셰이크 읽기가 WouldBlock으로 즉시 실패
                            //   (09-04 실기: 여는 쪽 성립 · 받는 쪽만 실패의 원인).
                            let _ = stream.set_nonblocking(false);
                            let ip = from.ip();
                            let cooled = cooldown
                                .lock()
                                .ok()
                                .and_then(|m| m.get(&ip).copied())
                                .is_some_and(|until| Instant::now() < until);
                            if cooled || INBOUND_HS.load(Ordering::Relaxed) >= INBOUND_MAX {
                                drop(stream); // 냉각 중이거나 폭주 — 조용히 닫는다
                                continue;
                            }
                            INBOUND_HS.fetch_add(1, Ordering::Relaxed);
                            let (id, proxy, dir) = (id.clone(), proxy.clone(), dir.clone());
                            let cooldown = cooldown.clone();
                            let _ = std::thread::Builder::new()
                                .name("nclip-lan-in".into())
                                .spawn(move || {
                                    let r = handshake(stream, &id, false);
                                    INBOUND_HS.fetch_sub(1, Ordering::Relaxed);
                                    match r {
                                        Ok(s) => {
                                            println!("LAN 직결: ← {from} 수락");
                                            crate::sync_cmd::run_peer(
                                                s, me, &LAN_GEN, gen, proxy, dir, "LAN",
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "LAN 직결: {from} 핸드셰이크 실패({e}) — {}초 냉각",
                                                FAIL_COOLDOWN.as_secs()
                                            );
                                            if let Ok(mut m) = cooldown.lock() {
                                                m.insert(ip, Instant::now() + FAIL_COOLDOWN);
                                                if m.len() > 256 {
                                                    m.clear();
                                                }
                                            }
                                        }
                                    }
                                });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(500)),
                    }
                }
            });
    }

    // ② 비콘 송수신.
    let sock = match beacon_socket() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("LAN 직결: 비콘 소켓 실패({e}) — 발견 없이(수락만) 진행");
            return;
        }
    };
    let _ = std::thread::Builder::new()
        .name("nclip-lan-beacon".into())
        .spawn(move || {
            let my_beacon = encode_beacon(&tags[1], &me_hex, port); // [1] = 오늘
            // 송신 대상: 제한 브로드캐스트 + 기본 경로 인터페이스의 /24 지향 브로드캐스트
            //   (가상 어댑터가 여럿이면 255.255.255.255가 엉뚱한 곳으로만 나가는 일이 있다).
            let targets = beacon_targets();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut last_send = Instant::now() - BEACON_EVERY;
            // 기기별 다음 다이얼 시각·간격(지수 백오프 8s → … → 5분 · 붙으면 초기화).
            let mut dial_next: std::collections::HashMap<String, (Instant, u32)> =
                std::collections::HashMap::new();
            let mut rx_sec = Instant::now();
            let mut rx_count = 0u32;
            let mut buf = [0u8; 128];
            println!("LAN 직결: 비콘 {BEACON_PORT}/udp · 수락 {port}/tcp — 같은 핸들·암호 기기를 찾습니다");
            while LAN_GEN.load(Ordering::Relaxed) == gen {
                // LAN 피어가 붙어 있으면 30초 — 발견은 이미 됐고 새 기기 등장만 잡으면 된다.
                let every = if crate::devices::online_via("LAN") > 0 {
                    BEACON_SLOW
                } else {
                    BEACON_EVERY
                };
                if last_send.elapsed() >= every {
                    for t in &targets {
                        if let Err(e) = sock.send_to(&my_beacon, t) {
                            eprintln!("LAN 직결: 비콘 송신 실패({t} · {e})");
                        }
                    }
                    last_send = Instant::now();
                }
                let (n, from) = match sock.recv_from(&mut buf) {
                    Ok(v) => v,
                    Err(_) => continue, // 타임아웃·잡음
                };
                // 초당 수신 상한 — 비콘 홍수(같은 포트에 쏟아지는 잡음)에 CPU를 내주지 않는다.
                if rx_sec.elapsed() >= Duration::from_secs(1) {
                    rx_sec = Instant::now();
                    rx_count = 0;
                }
                rx_count += 1;
                if rx_count > BEACON_RX_PER_SEC {
                    continue;
                }
                let Some((tag, hex, their_port)) = decode_beacon(&buf[..n]) else {
                    continue;
                };
                if hex == me_hex || !tags.contains(&tag) {
                    continue;
                }
                if seen.len() > 256 {
                    seen.clear();
                }
                if seen.insert(hex.clone()) {
                    println!("LAN 직결: 비콘 수신 — {}… @ {} (tcp {their_port})", &hex[..8], from.ip());
                }
                if crate::devices::is_online(&hex) {
                    dial_next.remove(&hex); // 붙었다 = 백오프 초기화
                    continue;
                }
                if me_hex >= hex {
                    continue; // 상대가 걸 차례(타이브레이크)
                }
                let now = Instant::now();
                // 기기별 실패 횟수 → 공용 재시도 정책(sync.retry) — 붙으면 초기화.
                let (next, fails) = dial_next.get(&hex).copied().unwrap_or((now, 0u32));
                if now < next {
                    continue;
                }
                if dial_next.len() > 256 {
                    dial_next.clear();
                }
                let wait = crate::sync_cmd::policy().wait(fails + 1);
                dial_next.insert(hex.clone(), (now + wait, fails + 1));
                let Some(peer) = nclip_sync::relay::parse_peer_hex(&hex) else {
                    continue;
                };
                let target = SocketAddr::new(from.ip(), their_port);
                let (id, proxy, dir) = (id.clone(), proxy.clone(), dir.clone());
                let _ = std::thread::Builder::new()
                    .name("nclip-lan-dial".into())
                    .spawn(move || {
                        let stream = match TcpStream::connect_timeout(&target, DIAL_TIMEOUT) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("LAN 직결: {target} 연결 실패({e})");
                                return;
                            }
                        };
                        let s = match handshake(stream, &id, true) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("LAN 직결: {target} 핸드셰이크 실패({e})");
                                return;
                            }
                        };
                        use nclip_sync::session::Session as _;
                        if s.peer() != peer {
                            eprintln!("LAN 직결: {target} 키 불일치 — 비콘과 다른 기기(버림)");
                            return;
                        }
                        println!("LAN 직결: → {target} 성립 ({}…)", &hex[..8]);
                        crate::sync_cmd::run_peer(s, me, &LAN_GEN, gen, proxy, dir, "LAN");
                    });
            }
        });
}

/// 비콘 송신 대상 — 255.255.255.255 + 기본 경로 로컬 IP의 /24 브로드캐스트(있으면).
fn beacon_targets() -> Vec<SocketAddr> {
    let mut v = vec![SocketAddr::from(([255, 255, 255, 255], BEACON_PORT))];
    if let Ok(probe) = UdpSocket::bind(("0.0.0.0", 0)) {
        if probe.connect(("8.8.8.8", 53)).is_ok() {
            if let Ok(SocketAddr::V4(local)) = probe.local_addr() {
                let o = local.ip().octets();
                v.push(SocketAddr::from(([o[0], o[1], o[2], 255], BEACON_PORT)));
            }
        }
    }
    v
}

/// TCP 스트림 → Noise 세션(여는 쪽 `initiate` / 받는 쪽 `accept` · 릴레이 종단과 같은 prologue).
fn handshake(
    stream: TcpStream,
    id: &nclip_sync::Identity,
    initiator: bool,
) -> Result<nclip_sync::NoiseSession<Box<dyn nclip_sync::Link>>, String> {
    let mut link = nclip_sync::tcp::TcpLink::new(stream).map_err(|e| format!("link: {e}"))?;
    link.set_poll_mode(HS_TIMEOUT)
        .map_err(|e| format!("poll: {e}"))?;
    let boxed: Box<dyn nclip_sync::Link> = Box::new(link);
    let mut s = if initiator {
        nclip_sync::NoiseSession::initiate_with_prologue(boxed, id, nclip_sync::relay::E2E_PROLOGUE)
            .map_err(|e| format!("initiate: {e:?}"))?
    } else {
        nclip_sync::NoiseSession::accept_with_prologue(boxed, id, nclip_sync::relay::E2E_PROLOGUE)
            .map_err(|e| format!("accept: {e:?}"))?
    };
    use nclip_sync::session::Session as _;
    s.set_recv_timeout(None);
    Ok(s)
}

#[allow(dead_code)]
fn _ip_is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => v.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 루프백 TCP로 여는 쪽/받는 쪽 핸드셰이크 — 실기에서 받는 쪽만 실패한 원인 확정용.
    #[test]
    fn loopback_handshake_both_sides() {
        let a = nclip_sync::Identity::generate();
        let b = nclip_sync::Identity::generate();
        let l = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let addr = l.local_addr().expect("addr");
        let acc = std::thread::spawn(move || {
            let (s, _) = l.accept().expect("accept");
            handshake(s, &a, false).map(|_| ())
        });
        let s = TcpStream::connect(addr).expect("connect");
        let ini = handshake(s, &b, true).map(|_| ());
        let acc = acc.join().expect("join");
        assert_eq!(ini, Ok(()), "initiator");
        assert_eq!(acc, Ok(()), "acceptor");
    }

    #[test]
    fn beacon_roundtrip_and_tag_is_not_rid() {
        let tags = lan_tags("kiros33", "pw");
        assert_eq!(tags.len(), 3);
        let rid = nclip_sync::rid::rids_around("kiros33", "pw")[1];
        assert_ne!(&tags[1][..], &rid[..], "태그는 RID를 그대로 싣지 않는다");
        let hex = "ab".repeat(32);
        let b = encode_beacon(&tags[0], &hex, 40123);
        let (t, h, p) = decode_beacon(&b).expect("decode");
        assert_eq!((t, h.as_str(), p), (tags[0], hex.as_str(), 40123));
        assert!(decode_beacon(&b[..50]).is_none());
        assert_ne!(
            lan_tags("kiros33", "other")[0],
            tags[0],
            "암호가 다르면 다른 태그"
        );
    }
}
