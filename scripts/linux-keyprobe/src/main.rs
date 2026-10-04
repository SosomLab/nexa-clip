//! ★ Linux 팝업 단축키 잔향(`c` 스톰) **자동 재현 하네스**(09-27 · T-15d 근인 확정용).
//!
//! 무엇을 하나: 포털 RemoteDesktop으로 전역 단축키(기본 Shift+Alt+C — evdev 42·56·46)를 **누르고 · 유지하고 ·
//! 지정한 순서로 떼면서**, ① XI2 raw 키 이벤트(X 서버가 처리한 모든 누름/뗌 · 오토리피트 포함)와
//! ② `QueryKeymap`(X 서버가 "눌려 있다"고 믿는 키)을 시각과 함께 기록하고 ③ 팝업 창 스크린샷을 남긴다.
//! 마지막에 고착이 남았으면 `c` 누름/뗌으로 풀고 Esc로 팝업을 닫아 원상 복구한다.
//!
//! 전제: 설치본(`/usr/bin/nexa-clip`)이 떠 있고 단축키가 등록돼 있을 것 · `xdotool`·ImageMagick `import`.
//! ★ 반드시 **앱 이름의 systemd 스코프 안에서** 띄운다 — 포털은 cgroup 유닛 이름에서 앱 id를 읽으므로 에디터
//!   터미널에서 그냥 실행하면 `com.microsoft.VSCode`로 잡혀 앱의 저장 토큰이 거부되고 **승인 대화창에서 멈춘다**.
//!
//! ```sh
//! cd scripts/linux-keyprobe && cargo build
//! systemd-run --user --scope --quiet --unit "app-gnome-nexa\x2dclip-$RANDOM.scope" \
//!   env NCLIP_RD_TOKEN=$HOME/.config/nexa-clip/portal-remotedesktop.token \
//!       KEYPROBE_SHOT=/tmp/keyprobe.png \
//!   target/debug/linux-keyprobe <hold_ms> <mods-first|c-first|all-at-once> <gap_ms> [observe_ms]
//! ```
//! 실측 09-27(mutter 50.1 · XWayland 24.1.10 · VMware 게스트): `300 c-first 60` = 고착 재현(수정 전 `cccc…`) ·
//! `300 mods-first 60` = 정상 · `80 …`(포커스 전 전부 해제) = 정상. 환경 변수:
//! - `KEYPROBE_UNSTICK=press-release|release|none` — 고착 해소 방식(기본 press-release · `release`는 안 풀리는 것을 실증)
//! - `KEYPROBE_ESC_FIRST=1` — 고착 확인 전에 팝업을 먼저 닫아 본다(닫히면 XWayland가 전부 해제하는지)
//! - `KEYPROBE_PRE=122` — 단축키를 누르기 **전에** evdev 키를 차례로 누름/뗌(10-04 — 지금 포커스 창에서 한/영 전환:
//!   ibus-hangul의 한글 상태는 창을 넘어 이어지고 팝업의 Esc가 그것을 끈다 → 한글 상태 고착을 반복 재현하려면 매 회차 다시 켠다)
//! - `KEYPROBE_CLOSE=toggle` — 끝에 Esc 대신 단축키를 다시 눌러 팝업을 닫는다(Esc가 ibus-hangul의 한글 상태를 끄지 않게)
//! - `KEYPROBE_TYPE=30,46,46` — 관찰 뒤·스크린샷 전에 evdev 키를 차례로 누름/뗌(고착 중에도 검색이 되는지 · `c` 첫 글자 손실)
//! 사용: linux-keyprobe <hold_ms> <order> <gap_ms> [observe_ms]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::{ConnectionExt as _, EventMask, XIEventMask};
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

const KEY_LEFTSHIFT: i32 = 42;
const KEY_LEFTALT: i32 = 56;
const KEY_C: i32 = 46;
const KEY_ESC: i32 = 1;

fn down_keys(conn: &RustConnection) -> Vec<u8> {
    let r = conn.query_keymap().unwrap().reply().unwrap();
    let mut v = Vec::new();
    for (i, b) in r.keys.iter().enumerate() {
        for bit in 0..8 {
            if b & (1 << bit) != 0 {
                v.push((i * 8 + bit) as u8);
            }
        }
    }
    v
}

fn popup_wid() -> Option<String> {
    let o = std::process::Command::new("xdotool")
        .args(["search", "--onlyvisible", "--name", "^Nexa Clip$"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout);
    s.lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hold_ms: u64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(300);
    let order = a.get(2).cloned().unwrap_or_else(|| "mods-first".into());
    let gap_ms: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(60);
    let observe_ms: u64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(2500);
    if let Some(t) = std::env::var_os("NCLIP_RD_TOKEN") {
        nclip_plat::remote_input_linux::configure_token_path(t.into());
    }
    let t0 = Instant::now();
    let ms = move || t0.elapsed().as_millis();

    // ── XI2 raw 키 로거(스레드) ──
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let logger = std::thread::spawn(move || {
        let (conn, screen) = RustConnection::connect(None).expect("X 연결");
        let root = conn.setup().roots[screen].root;
        conn.xinput_xi_query_version(2, 2).unwrap().reply().unwrap();
        conn.xinput_xi_select_events(
            root,
            &[EventMask {
                deviceid: 1, // XIAllMasterDevices
                mask: vec![XIEventMask::RAW_KEY_PRESS | XIEventMask::RAW_KEY_RELEASE],
            }],
        )
        .unwrap()
        .check()
        .unwrap();
        conn.flush().unwrap();
        let mut n_press: u32 = 0;
        let mut n_press_c: u32 = 0;
        while !stop2.load(Ordering::Relaxed) {
            match conn.poll_for_event() {
                Ok(Some(Event::XinputRawKeyPress(e))) => {
                    n_press += 1;
                    if e.detail == 54 {
                        n_press_c += 1;
                    }
                    // 로그 폭주 방지: c 리피트는 10개마다 한 줄
                    if e.detail != 54 || n_press_c <= 5 || n_press_c % 10 == 0 {
                        println!(
                            "[{:>5}ms] X RawKeyPress   kc={} flags={:?} time={} (c누름 누계 {})",
                            ms(),
                            e.detail,
                            e.flags,
                            e.time,
                            n_press_c
                        );
                    }
                }
                Ok(Some(Event::XinputRawKeyRelease(e))) => {
                    println!(
                        "[{:>5}ms] X RawKeyRelease kc={} flags={:?} time={}",
                        ms(),
                        e.detail,
                        e.flags,
                        e.time
                    );
                }
                Ok(Some(_)) => {}
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(e) => {
                    println!("logger err {e}");
                    break;
                }
            }
        }
        println!(
            "[{:>5}ms] 로거 종료 — RawKeyPress 총 {n_press} · 그중 c(54) {n_press_c}",
            ms()
        );
    });
    std::thread::sleep(Duration::from_millis(200));
    let (qconn, _) = RustConnection::connect(None).expect("X 연결");
    use nclip_plat::remote_input_linux::key_seq;
    if popup_wid().is_some() {
        println!("[{:>5}ms] 시작 전 팝업이 열려 있어 Esc로 닫는다", ms());
        key_seq(&[(KEY_ESC, true, 30), (KEY_ESC, false, 0)]).expect("esc");
        std::thread::sleep(Duration::from_millis(500));
    }
    println!(
        "[{:>5}ms] 시작 전 눌린 키 {:?} · 팝업 {:?}",
        ms(),
        down_keys(&qconn),
        popup_wid()
    );

    // ── 주입 ──
    if let Ok(codes) = std::env::var("KEYPROBE_PRE") {
        let steps: Vec<(i32, bool, u64)> = codes
            .split(',')
            .filter_map(|c| c.trim().parse::<i32>().ok())
            .flat_map(|c| [(c, true, 30), (c, false, 30)])
            .collect();
        key_seq(&steps).expect("pre");
        std::thread::sleep(Duration::from_millis(400));
        println!("[{:>5}ms] 사전 키 주입({codes})", t0.elapsed().as_millis());
    }
    println!(
        "[{:>5}ms] 주입: Shift↓ Alt↓ C↓ (hold {hold_ms}ms · order {order} · gap {gap_ms}ms)",
        ms()
    );
    key_seq(&[
        (KEY_LEFTSHIFT, true, 15),
        (KEY_LEFTALT, true, 15),
        (KEY_C, true, 0),
    ])
    .expect("press");
    // 팝업 창 등장 시각 기록(폴링)
    let hold_deadline = Instant::now() + Duration::from_millis(hold_ms);
    let mut seen = None;
    while Instant::now() < hold_deadline {
        if seen.is_none() {
            if let Some(w) = popup_wid() {
                println!("[{:>5}ms] 팝업 창 보임 wid={w}", ms());
                seen = Some(w);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "[{:>5}ms] 해제 시작 · 눌린 키 {:?}",
        ms(),
        down_keys(&qconn)
    );
    match order.as_str() {
        "c-first" => key_seq(&[
            (KEY_C, false, gap_ms),
            (KEY_LEFTALT, false, gap_ms),
            (KEY_LEFTSHIFT, false, 0),
        ])
        .expect("rel"),
        "all-at-once" => key_seq(&[
            (KEY_C, false, 0),
            (KEY_LEFTALT, false, 0),
            (KEY_LEFTSHIFT, false, 0),
        ])
        .expect("rel"),
        _ => key_seq(&[
            (KEY_LEFTALT, false, gap_ms),
            (KEY_LEFTSHIFT, false, gap_ms),
            (KEY_C, false, 0),
        ])
        .expect("rel"),
    }
    println!("[{:>5}ms] 해제 완료(키보드 다 뗌)", ms());
    if seen.is_none() {
        if let Some(w) = popup_wid() {
            println!("[{:>5}ms] 팝업 창 보임(늦게) wid={w}", ms());
            seen = Some(w);
        }
    }

    // ── 관찰 ──
    let obs_end = Instant::now() + Duration::from_millis(observe_ms);
    while Instant::now() < obs_end {
        std::thread::sleep(Duration::from_millis(500));
        println!(
            "[{:>5}ms] 서버 눌린 키(QueryKeymap) {:?} · 활성창 {:?}",
            ms(),
            down_keys(&qconn),
            popup_wid()
        );
    }
    if let Ok(codes) = std::env::var("KEYPROBE_TYPE") {
        let steps: Vec<(i32, bool, u64)> = codes
            .split(',')
            .filter_map(|c| c.trim().parse::<i32>().ok())
            .flat_map(|c| [(c, true, 40), (c, false, 120)])
            .collect();
        println!(
            "[{:>5}ms] 타이핑 주입(evdev {codes}) · 주입 전 눌린 키 {:?}",
            ms(),
            down_keys(&qconn)
        );
        key_seq(&steps).expect("type");
        std::thread::sleep(Duration::from_millis(400));
        println!("[{:>5}ms] 타이핑 뒤 눌린 키 {:?}", ms(), down_keys(&qconn));
    }
    if let Some(w) = &seen {
        let out = std::env::var("KEYPROBE_SHOT").unwrap_or_else(|_| "/tmp/keyprobe.png".into());
        let st = std::process::Command::new("import")
            .args(["-window", w, &out])
            .status();
        println!("[{:>5}ms] 스크린샷 {out} → {st:?}", ms());
    }
    // ── 정리(변형 실험) ──
    let unstick = std::env::var("KEYPROBE_UNSTICK").unwrap_or_else(|_| "press-release".into());
    let esc_first = std::env::var("KEYPROBE_ESC_FIRST").is_ok();
    let esc = |label: &str| {
        if popup_wid().is_some() {
            // ★ `KEYPROBE_CLOSE=toggle`(10-04) — Esc 대신 단축키를 다시 눌러 닫는다(수식키 먼저 떼는 정상 순서).
            //   ibus-hangul은 Esc를 "한글 끄기"로 써서(off-keys) 다음 회차가 영문 상태로 시작한다 — 한글 상태 반복 재현용.
            if std::env::var("KEYPROBE_CLOSE").as_deref() == Ok("toggle") {
                key_seq(&[
                    (KEY_LEFTSHIFT, true, 20),
                    (KEY_LEFTALT, true, 20),
                    (KEY_C, true, 120),
                    (KEY_LEFTSHIFT, false, 20),
                    (KEY_LEFTALT, false, 20),
                    (KEY_C, false, 0),
                ])
                .expect("toggle-close");
            } else {
                key_seq(&[(KEY_ESC, true, 30), (KEY_ESC, false, 0)]).expect("esc");
            }
            std::thread::sleep(Duration::from_millis(400));
            println!(
                "[{:>5}ms] {label}: Esc 뒤 팝업 {:?} · 눌린 키 {:?}",
                ms(),
                popup_wid(),
                down_keys(&qconn)
            );
        }
    };
    if esc_first {
        esc("T3");
        std::thread::sleep(Duration::from_millis(600));
        println!(
            "[{:>5}ms] T3: 팝업 닫힌 뒤 0.6s · 눌린 키 {:?}",
            ms(),
            down_keys(&qconn)
        );
    }
    let stuck = down_keys(&qconn);
    if stuck.contains(&54) {
        match unstick.as_str() {
            "release" => {
                println!("[{:>5}ms] ★ c 고착 → **해제만** 주입", ms());
                key_seq(&[(KEY_C, false, 0)]).expect("unstick");
                std::thread::sleep(Duration::from_millis(250));
                let now = down_keys(&qconn);
                println!("[{:>5}ms] 해제만 주입 뒤 눌린 키 {:?}", ms(), now);
                if now.contains(&54) {
                    println!("[{:>5}ms] 여전히 고착 → 누름/뗌으로 강제 해제", ms());
                    key_seq(&[(KEY_C, true, 30), (KEY_C, false, 0)]).expect("unstick2");
                    std::thread::sleep(Duration::from_millis(250));
                    println!(
                        "[{:>5}ms] 강제 해제 뒤 눌린 키 {:?}",
                        ms(),
                        down_keys(&qconn)
                    );
                }
            }
            "none" => println!("[{:>5}ms] ★ c 고착 — 그대로 둠(요청)", ms()),
            _ => {
                println!(
                    "[{:>5}ms] ★ c(54) 고착 확인 → 풀기 위해 c 누름/뗌 주입",
                    ms()
                );
                key_seq(&[(KEY_C, true, 30), (KEY_C, false, 0)]).expect("unstick");
                std::thread::sleep(Duration::from_millis(250));
                println!("[{:>5}ms] 풀기 뒤 눌린 키 {:?}", ms(), down_keys(&qconn));
            }
        }
    } else {
        println!("[{:>5}ms] 고착 없음(눌린 키 {:?})", ms(), stuck);
    }
    esc("끝");
    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::Relaxed);
    let _ = logger.join();
}
