//! `tray` — ★ **상주 셸**(T-12e·T-12e2). 트레이 + 감시 + **설정 창 열고 닫기**.
//!
//! 24시간 상주 제품의 셸이다. 시작은 **트레이만**(창 없음), 트레이 좌클릭/"열기"로
//! 설정 창이 열리고, 닫으면 `ui.close_to_tray`(기본 꺼짐)에 따라 **종료** 또는
//! **트레이로 숨기**(08-28 사용자 요청 · beep 동일). 종료는 트레이 메뉴에서.
//!
//! ## 구조 — winit 한 루프에 전부
//!
//! winit `EventLoop`는 프로세스당 하나·메인 스레드 강제다. 트레이(전용 스레드)와
//! 감시(전용 스레드)는 [`EventLoopProxy`]로 사용자 이벤트를 쏘고, 메인 루프가
//! 창([`crate::settings_win::App`])·툴팁 갱신·종료를 한 곳에서 처리한다(beep 호스트 문법).
//!
//! ## 자동 시작(`app.autostart` · 기본 켜짐)
//!
//! 시작마다 [`nclip_plat::autostart::boot_sync`]로 재동기화한다 — 포터블 이동로
//! 옛 경로가 죽는 것을 막고, ★ **사용자가 앱 밖에서 지운 등록은 존중**한다
//! (설정을 끔으로 내려 화면과 실제를 일치시킨다).

use crate::conf::Settings;
use crate::settings_win::App;
use crate::watch_cmd::Gate;
use nclip_core::capture::{clip_text, summarize};
use nclip_core::history::{History, Pushed};
use nclip_core::{
    current_lang, has_content, tr, ClipSnapshot, ClipboardWatch as _, Msg, WatchCapability,
};
use nclip_gfx::Font;
use nclip_plat::autostart::{apply, boot_sync, is_registered, BootSync};
use nclip_plat::tray::{spawn, TrayContent, TrayEvent, TrayHandle};
use nclip_plat::watch::PlatformWatch;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// 아이콘 한 변(px) — 트레이 표준.
const ICON_SIDE: u32 = 32;

/// 트레이 메뉴 라벨 최대 글자 수 — 길면 메뉴가 화면을 덮는다(문자 경계 절단).
const MENU_LABEL_CHARS: usize = 44;

/// 다른 스레드(트레이·감시)에서 메인 루프로 쏘는 사건.
#[derive(Debug)]
enum ShellEvent {
    /// 트레이 좌클릭·메뉴 "열기" — 설정 창을 연다(있으면 앞으로).
    Open,
    /// 트레이 메뉴 "종료".
    Quit,
    /// 감시가 항목을 잡았다 — 게이트를 지나면 이력에 넣는다.
    Captured(Box<ClipSnapshot>),
    /// ★ 최근 항목 선택(T-18e) — 그 항목의 표현 전부를 클립보드로 되돌린다.
    Recent(usize),
}

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

/// 툴팁 문자열 — 보관 수를 함께 보여 준다(감시가 실제로 도는 것이 보인다).
fn tooltip(held: usize) -> String {
    if held == 0 {
        "Nexa Clip".to_string()
    } else {
        format!("Nexa Clip — {held}개 보관")
    }
}

fn content(held: usize, recent: Vec<String>) -> TrayContent {
    let lang = current_lang();
    TrayContent {
        rgba: icon_rgba(),
        side: ICON_SIDE,
        tooltip: tooltip(held),
        name: tr(lang, Msg::AppName).to_string(),
        open_label: tr(lang, Msg::TrayOpen).to_string(),
        quit_label: tr(lang, Msg::TrayQuit).to_string(),
        recent,
    }
}

/// 시작마다 자동 시작 등록을 설정값과 동기화한다(멱등 · 외부 삭제 존중).
fn sync_autostart(conf: &mut Settings) {
    let want = conf.state.get("app.autostart") == "on";
    let was = conf.state.get("app.autostart_reg") == "on";
    let now = Instant::now();
    match boot_sync(want, was, is_registered()) {
        BootSync::Register => match apply(true) {
            Ok(()) => {
                conf.set("app.autostart_reg", "on".into(), now);
                println!("자동 시작: 등록 동기화 (로그인 시 실행 · 현재 경로)");
            }
            Err(e) => eprintln!("자동 시작 등록 실패: {e} — 다음 시작에서 재시도합니다"),
        },
        BootSync::Unregister => {
            if let Err(e) = apply(false) {
                eprintln!("자동 시작 해제 실패: {e}");
            } else if was {
                conf.set("app.autostart_reg", "off".into(), now);
            }
        }
        BootSync::RespectRemoval => {
            // ★ 사용자가 레지스트리 편집기·정리 도구로 지운 의사를 존중한다 —
            //   무조건 재등록하면 사용자와 앱의 줄다리기가 된다(beep 규약).
            conf.set("app.autostart", "off".into(), now);
            conf.set("app.autostart_reg", "off".into(), now);
            println!("자동 시작: 앱 밖에서 해제된 것을 발견 — 설정을 끔으로 맞춥니다");
        }
    }
}

/// 상주 셸 — 창·트레이·감시·이력을 winit 한 루프로.
struct Shell {
    app: App,
    tray: TrayHandle,
    /// ★ 세션 이력(T-13 1단) — 트레이 최근 메뉴와 재적재의 원천.
    history: History,
    /// 수집 게이트 — `watch` 진단과 같은 정책(민감·제외 앱·브라우저 암호).
    gate: Gate,
    /// 트레이 메뉴에 보일 최근 개수(`ui.tray_recent_n`).
    tray_n: usize,
}

impl Shell {
    /// 이력이 변했다 — 트레이 메뉴·툴팁을 새 내용으로.
    fn refresh_tray(&self) {
        self.tray.update(content(
            self.history.len(),
            self.history.recent_labels(self.tray_n),
        ));
    }
}

impl ApplicationHandler<ShellEvent> for Shell {
    fn resumed(&mut self, _el: &ActiveEventLoop) {
        // ★ 시작은 트레이만 — 창은 트레이 "열기"에서 연다(상주 앱의 조용한 출발).
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        ApplicationHandler::window_event(&mut self.app, el, id, event);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: ShellEvent) {
        match ev {
            ShellEvent::Open => self.app.ensure_window(el),
            ShellEvent::Quit => el.exit(),
            ShellEvent::Captured(snap) => {
                // 게이트 — `watch`와 같은 정책. 막힌 것은 이력에도 안 들어간다.
                if self.gate.blocks(&snap).is_some() {
                    return;
                }
                let (kind, line) = summarize(&snap);
                let label = clip_text(&line, MENU_LABEL_CHARS);
                // ★ 재적재로 되돌아온 우리 게시도 여기로 온다 — 승격(맨 위로)이
                //   곧 에코 처리다(항목이 늘지 않는다).
                let _pushed: Pushed = self.history.push(&snap, kind, label);
                self.refresh_tray();
            }
            ShellEvent::Recent(i) => {
                let Some(item) = self.history.get(i) else {
                    return;
                };
                match nclip_plat::clipboard::set_reps(&item.reps) {
                    Ok(n) => println!("재적재: \"{}\" — 표현 {n}개 게시", item.label),
                    Err(e) => eprintln!("재적재 실패: {e}"),
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        ApplicationHandler::about_to_wait(&mut self.app, el);
    }
}

/// 트레이 + 감시 + 설정 창 상주. 종료는 트레이 메뉴에서.
pub(crate) fn run() {
    let Some((data, idx)) = nclip_plat::font::system_ui_font() else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다.");
        std::process::exit(1);
    };
    let font = match Font::from_static(data, idx) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("폰트 로드 실패: {e:?}");
            std::process::exit(1);
        }
    };

    let mut conf = Settings::load();
    sync_autostart(&mut conf);

    let Ok(el) = EventLoop::<ShellEvent>::with_user_event().build() else {
        eprintln!("이벤트 루프 생성 실패");
        std::process::exit(1);
    };
    el.set_control_flow(ControlFlow::Wait);

    // 트레이 — 이벤트는 프록시로 메인 루프에 되돌린다(beep 호스트 문법).
    let proxy = el.create_proxy();
    let Some(tray) = spawn(content(0, Vec::new()), move |ev| {
        let _ = proxy.send_event(match ev {
            TrayEvent::Quit => ShellEvent::Quit,
            TrayEvent::Recent(i) => ShellEvent::Recent(i),
            TrayEvent::Open | TrayEvent::OpenTarget(_) => ShellEvent::Open,
        });
    }) else {
        eprintln!("트레이를 띄울 수 없습니다(이 OS는 아직 미이식 — docs/21 참조).");
        std::process::exit(1);
    };

    // 감시 — 스냅숏을 통째로 셸에 넘긴다(게이트·이력은 메인 루프가).
    let mut watch = PlatformWatch::new();
    if matches!(watch.capability(), WatchCapability::Supported { .. }) {
        let proxy = el.create_proxy();
        match watch.start(Box::new(move |snap| {
            if has_content(&snap.reps) {
                let _ = proxy.send_event(ShellEvent::Captured(Box::new(snap)));
            }
        })) {
            Ok(()) => println!("클립보드 감시: ok — 잡힌 항목이 트레이 메뉴에 쌓입니다"),
            Err(e) => eprintln!("클립보드 감시 시작 실패: {e:?} — 트레이만 동작합니다"),
        }
    } else {
        println!("클립보드 감시: 이 OS는 미구현 — 트레이만 동작합니다");
    }

    println!("트레이 상주: ok — 좌클릭/열기 = 설정 창 · 우클릭 = 최근 항목 메뉴(클릭 = 재적재)");
    println!(
        "창 닫기: {}",
        if conf.state.get("ui.close_to_tray") == "on" {
            "트레이로 숨김 (ui.close_to_tray = on)"
        } else {
            "앱 종료 (설정 '창을 닫아도 트레이에 남기'를 켜면 숨김)"
        }
    );

    // 이력 상한·메뉴 개수는 설정에서 — 세션 동안 고정(설정 즉시 반영은 후속).
    let cap: usize = conf.state.get("store.max_items").parse().unwrap_or(1000);
    let tray_n: usize = conf.state.get("ui.tray_recent_n").parse().unwrap_or(8);
    let gate = Gate::from_state(&conf);

    let mut shell = Shell {
        app: App::new(font, conf, true),
        tray,
        history: History::new(cap),
        gate,
        tray_n,
    };
    if let Err(e) = el.run_app(&mut shell) {
        eprintln!("이벤트 루프 오류: {e}");
    }
    // ★ 종료 직전 강제 수거 — "바꾸고 바로 종료하면 안 저장됨"을 막는다.
    if shell.app.conf.flush() {
        println!("설정 저장: {}", shell.app.conf.path().display());
    }
    println!("종료합니다 — 이번 상주에서 {}개 보관.", shell.history.len());
}
