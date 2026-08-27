//! `tray` — ★ **트레이 상주 1단**(T-12e). 아이콘 + 메뉴 골격 + 감시 통합.
//!
//! 24시간 상주 제품의 첫 상주 모양이다. 감시([`crate::watch_cmd`]와 같은 파이프)를
//! 백그라운드로 돌리고, 잡힌 항목 수를 **툴팁에 실시간 반영**한다 — `update()` 경로가
//! 실제로 도는지 눈으로 확인된다.
//!
//! | 조작 | 동작 |
//! |---|---|
//! | 좌클릭 · 메뉴 "열기" | 풍선 알림(메인창은 T-18b에서) — 알림 경로 실증 |
//! | 메뉴 "종료" | 프로세스 종료 |
//!
//! 최근 N개 메뉴(T-18e)는 저장소(T-16)가 생긴 뒤 붙는다 — 빈 메뉴를 먼저 그리지 않는다.

use nclip_core::{current_lang, tr, ClipboardWatch as _, Msg, WatchCapability};
use nclip_plat::tray::{spawn, TrayContent, TrayEvent};
use nclip_plat::watch::PlatformWatch;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

/// 아이콘 한 변(px) — 트레이 표준.
const ICON_SIDE: u32 = 32;

/// ★ 계열 아이콘을 코드로 그린다 — 라운드 스퀘어 + 청록 세로 그라디언트(`#22C3D6→#0B7FA6`)
/// + 흰 클립보드 모티프. 애셋 파일을 링크에 끌어들이지 않는다(단일 바이너리).
fn icon_rgba() -> Vec<u8> {
    const TOP: (u8, u8, u8) = (0x22, 0xC3, 0xD6);
    const BOT: (u8, u8, u8) = (0x0B, 0x7F, 0xA6);
    let s = ICON_SIDE as i32;
    let radius = 7i32;
    let mut out = Vec::with_capacity((s * s * 4) as usize);
    for y in 0..s {
        // 세로 그라디언트.
        let t = y as u32;
        let lerp = |a: u8, b: u8| -> u8 {
            ((u32::from(a) * (ICON_SIDE - 1 - t) + u32::from(b) * t) / (ICON_SIDE - 1)) as u8
        };
        let (r, g, b) = (lerp(TOP.0, BOT.0), lerp(TOP.1, BOT.1), lerp(TOP.2, BOT.2));
        for x in 0..s {
            // 라운드 스퀘어 밖은 투명 — 네 모서리에서 반지름 검사.
            let cx = if x < radius {
                radius - 1 - x
            } else if x >= s - radius {
                x - (s - radius)
            } else {
                -1
            };
            let cy = if y < radius {
                radius - 1 - y
            } else if y >= s - radius {
                y - (s - radius)
            } else {
                -1
            };
            let outside = cx >= 0 && cy >= 0 && cx * cx + cy * cy > radius * radius;
            if outside {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // 흰 전경 — 클립보드 판(세로 직사각) + 상단 집게(가로 막대).
            let board = (8..24).contains(&x) && (10..26).contains(&y);
            let board_inner = (10..22).contains(&x) && (12..24).contains(&y);
            let clip = (12..20).contains(&x) && (6..11).contains(&y);
            if (board && !board_inner) || clip {
                out.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    out
}

/// 툴팁 문자열 — 수집 수를 함께 보여 준다(감시가 실제로 도는 것이 보인다).
fn tooltip(count: u32) -> String {
    if count == 0 {
        "Nexa Clip".to_string()
    } else {
        format!("Nexa Clip — {count}개 수집")
    }
}

fn content(count: u32) -> TrayContent {
    let lang = current_lang();
    TrayContent {
        rgba: icon_rgba(),
        side: ICON_SIDE,
        tooltip: tooltip(count),
        name: tr(lang, Msg::AppName).to_string(),
        open_label: tr(lang, Msg::TrayOpen).to_string(),
        quit_label: tr(lang, Msg::TrayQuit).to_string(),
    }
}

/// 트레이 + 감시 상주. 메뉴 "종료"로 끝낸다.
pub(crate) fn run() {
    // 감시 — 되면 항목 수를 세고, 안 되면(미구현 OS) 트레이만 띄운다(정직 표시).
    let mut watch = PlatformWatch::new();
    let watching = matches!(watch.capability(), WatchCapability::Supported { .. });

    let (quit_tx, quit_rx) = mpsc::channel::<()>();
    static COUNT: AtomicU32 = AtomicU32::new(0);

    let Some(handle) = spawn(content(0), {
        move |ev| match ev {
            TrayEvent::Quit => {
                let _ = quit_tx.send(());
            }
            TrayEvent::Open | TrayEvent::OpenTarget(_) => {
                // 메인창(T-18b) 전까지는 정직하게 알린다 — 조용히 무시하지 않는다.
                println!("트레이: 열기 — 메인창은 아직 없습니다(T-18b)");
            }
        }
    }) else {
        eprintln!("트레이를 띄울 수 없습니다(이 OS는 아직 미이식 — docs/21 참조).");
        std::process::exit(1);
    };

    println!("트레이 상주: ok — 우클릭 메뉴(열기/종료) · 좌클릭 = 열기");
    if watching {
        // ★ 감시 콜백은 개수만 늘리고 툴팁 갱신을 요청한다 — 무거운 일은 하지 않는다.
        //   (저장·목록은 T-16·T-18에서 이 자리에 붙는다.)
        let started = watch.start(Box::new(move |snap| {
            if !nclip_core::has_content(&snap.reps) {
                return;
            }
            let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            // TrayHandle은 스레드를 건너지 못하므로 여기서 새 내용만 만든다 —
            // 반영은 아래 메인 루프가 한다(200ms 내 — 사람 눈에는 즉시).
            let _ = n;
        }));
        match started {
            Ok(()) => println!("클립보드 감시: ok — 수집 수가 트레이 툴팁에 반영됩니다"),
            Err(e) => eprintln!("클립보드 감시 시작 실패: {e:?} — 트레이만 동작합니다"),
        }
    } else {
        println!("클립보드 감시: 이 OS는 미구현 — 트레이만 동작합니다");
    }
    println!("종료: 트레이 우클릭 → 종료 (또는 Ctrl+C)");

    // 메인 루프 — 종료 신호를 기다리며 툴팁을 갱신한다(변했을 때만).
    let mut shown = 0u32;
    loop {
        match quit_rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(()) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = COUNT.load(Ordering::Relaxed);
                if now != shown {
                    shown = now;
                    handle.update(content(now));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    println!("종료합니다 — 이번 상주에서 {shown}개 수집.");
}
