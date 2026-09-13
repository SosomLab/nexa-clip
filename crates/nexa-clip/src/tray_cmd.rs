//! `tray` — ★ **상주 셸**(T-12e·T-12e2). 트레이 + 감시 + **설정 창 열고 닫기**.
//!
//! 24시간 상주 제품의 셸이다. 시작은 **트레이만**(창 없음), 트레이 좌클릭/"열기"로
//! 설정 창이 열리고, 닫으면 `ui.close_to_tray`(기본 켜짐 · 08-30 정정)에 따라 **종료** 또는
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
use crate::main_win::{MainAction, MainWin};
use crate::popup_win::{Popup, PopupAction};
use crate::settings_win::App;
use crate::watch_cmd::Gate;
use nclip_core::capture::{clip_text, summarize};
use nclip_core::history::{History, HistoryItem, Pushed};
use nclip_core::{
    current_lang, has_content, tr, ClipSnapshot, ClipboardWatch as _, Msg, PasteAs,
    PasteInjector as _, RawRep, WatchCapability,
};
// Font는 conf::load_ui_font가 만들어 준다(폴백 체인 포함).
use nclip_plat::autostart::{apply, boot_sync, is_registered, BootSync};
use nclip_plat::paste::PlatformPaste;
use nclip_plat::tray::{spawn, TrayContent, TrayEvent, TrayHandle};
use nclip_plat::watch::PlatformWatch;
use nclip_store::{FileStore, HistoryStore, NullStore, StoredItem};
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::icon::{icon_rgba, ICON_SIDE};

/// 트레이 메뉴 라벨 최대 글자 수 — 길면 메뉴가 화면을 덮는다(문자 경계 절단).
const MENU_LABEL_CHARS: usize = 44;

/// ★ 원격 수신 표식(09-04 사용자 "전송 받은 클립보드는 플래그를 달고 재전송 금지") —
/// `source_app` 접두가 곧 영속 플래그다: 저장·복원·목록 우측 출처 표시(main_win)까지
/// 스키마 변경 없이 함께 온다. 이 표식이 붙은 항목은 승격 에코라도 **되돌려 보내지 않는다**.
const REMOTE_MARK: &str = "⇄ ";

/// 목록 썸네일 긴 변(px) — 팝업 행(30px)에 들어가는 크기의 2배(고DPI 여유).
/// ★ 09-02: 48→160 — Rich 본문 존(≈행 높이)에 그려도 흐릿하지 않게.
const THUMB_SIDE: u32 = 384; // ★ 09-04 DR-42 ⑤ C: 512→384(표시 상한 200px · 장당 1MB→0.6MB). 09-02 가변 행 선명도 유지.
/// ★ 미리보기 원본 디코드 상한(09-02 K4) — 패널 표시용이라 이 이상은 낭비.
const PREVIEW_SIDE: u32 = 1600;

/// 이미지 항목의 목록 썸네일 — `ui.image_preview`가 켜졌을 때만 호출된다.
///
/// DIB는 순수 변환([`nclip_core::img`] — 프로세스 없음), PNG는 **격리 워커**
/// ([`nclip_plat::imgdec`] — 파서는 본체에 없다). 실패는 `None` = 글리프 폴백.
fn make_thumb(reps: &[RawRep]) -> Option<(u32, u32, Vec<u8>)> {
    decode_image(reps, THUMB_SIDE)
}

/// 이미지 표현 → RGBA(긴 변 `side` 이하) — 썸네일·미리보기 공용(09-02 K4).
///
/// ★ 후보를 순위순으로 **전부** 시도한다(09-02 실기 — PPT의 PNG는 워커가 못 읽어도
/// CF_DIB·EMF가 멀쩡하다: 단일 후보 포기가 `thumb=-`의 원인이었다).
fn decode_image(reps: &[RawRep], side: u32) -> Option<(u32, u32, Vec<u8>)> {
    for i in nclip_core::capture::thumbnail_sources(reps) {
        if let Some(out) = decode_one(&reps[i], side) {
            return Some(out);
        }
    }
    None
}

/// 표현 하나를 디코드 — 실패는 다음 후보의 몷이다.
fn decode_one(r: &RawRep, side: u32) -> Option<(u32, u32, Vec<u8>)> {
    use nclip_core::img::{dib_to_rgba, downscale_rgba};
    match r.format.as_str() {
        "CF_DIB" | "CF_DIBV5" => {
            let (w, h, rgba) = dib_to_rgba(&r.data)?;
            downscale_rgba(w, h, &rgba, side)
        }
        "image/bmp" if r.data.len() > 14 => {
            let (w, h, rgba) = dib_to_rgba(&r.data[14..])?;
            downscale_rgba(w, h, &rgba, side)
        }
        "PNG" | "public.png" | "image/png" => nclip_plat::imgdec::decode_isolated(&r.data, side),
        // ★ EMF = GDI 래스터화(09-02 — PPT 글상자 서식 그대로). Windows 전용.
        #[cfg(target_os = "windows")]
        "CF_ENHMETAFILE" if !r.data.is_empty() => nclip_plat::emf::emf_to_rgba(&r.data, side),
        _ => None,
    }
}

/// ★ 창 기하를 **보이는 모니터 안**으로 되민다(09-04 사용자 — "2모니터에서 1모니터로 바뀌면 화면에 보이게"):
/// 창 중심이 든 모니터가 있으면 그 안으로 클램프, 없으면 주 모니터 좌상단 + 40px(크기는 모니터에 맞춰 줄임).
fn fit_geom_to_monitors(
    el: &ActiveEventLoop,
    (x, y, w, h): (i32, i32, u32, u32),
) -> (i32, i32, u32, u32) {
    #[allow(clippy::cast_possible_wrap)]
    let (cx, cy) = (x + w as i32 / 2, y + h as i32 / 2);
    let contains = |m: &winit::monitor::MonitorHandle| {
        let p = m.position();
        let s = m.size();
        cx >= p.x
            && cy >= p.y
            && cx < p.x + i32::try_from(s.width).unwrap_or(i32::MAX)
            && cy < p.y + i32::try_from(s.height).unwrap_or(i32::MAX)
    };
    let hit = el.available_monitors().find(contains);
    let lost = hit.is_none();
    let Some(mon) = hit.or_else(|| el.primary_monitor()) else {
        return (x, y, w, h);
    };
    let (mp, ms) = (mon.position(), mon.size());
    let (mw, mh) = (ms.width.max(200), ms.height.max(200));
    let (w2, h2) = (w.min(mw), h.min(mh));
    #[allow(clippy::cast_possible_wrap)]
    let (mw_i, mh_i, w_i, h_i) = (mw as i32, mh as i32, w2 as i32, h2 as i32);
    let (nx, ny) = if lost {
        #[allow(clippy::cast_possible_truncation)]
        let off = (40.0 * mon.scale_factor()).round() as i32;
        (
            (mp.x + off).min(mp.x + mw_i - w_i),
            (mp.y + off).min(mp.y + mh_i - h_i),
        )
    } else {
        (
            x.clamp(mp.x, (mp.x + mw_i - w_i).max(mp.x)),
            y.clamp(mp.y, (mp.y + mh_i - h_i).max(mp.y)),
        )
    };
    if lost {
        println!("메인창 위치 보정: 저장 위치({x},{y})가 모니터 밖 → ({nx},{ny})");
    }
    (nx, ny, w2, h2)
}

/// 검색 색인 배경 로더 작업 단위 — (항목 id, 라벨, 텍스트 blob 참조들(형식·id·길이)).
type PendingText = (u64, String, Vec<(String, [u8; 32], u64)>);

/// 다른 스레드(트레이·감시)에서 메인 루프로 쏘는 사건.
#[derive(Debug)]
/// [`Shell::resolve_remote_files`]의 답.
enum Resolved {
    Ready(Vec<RawRep>),
    Fetching,
    Text(Vec<RawRep>),
}

pub(crate) enum ShellEvent {
    /// 트레이 좌클릭·메뉴 "열기" — 설정 창을 연다(있으면 앞으로).
    Open,
    /// ★ 붙여넣기 스택(09-03 ③ · 09-08 원본/평문 × 줄바꿈) — 팝업이 닫힌 다음 바퀴에 순차 주입.
    PasteStack {
        /// 담은 순서의 항목 id.
        ids: Vec<u64>,
        /// 원본 또는 평문.
        as_: PasteAs,
        /// 항목 사이에 Enter 한 번.
        newline: bool,
    },
    /// ★ 동기화 릴레이 연결 상태(09-03) — 트레이 점·메인 인디케이터 갱신.
    SyncState(bool),
    /// ★ 러너 상태만 바뀜(접속 중·실패·중단) — 루프를 깨워 설정 창 폴링을 돌린다(09-03).
    SyncTick,
    /// ★ 다른 기기의 클립보드 항목(09-04 · DR-6) — 승인된 기기에서 온 것. **디코드·OS 표현 변환·에코
    ///   지문까지 세션 스레드가 끝내고** 온다(UI 스레드는 이력 등재·게시만 — "네트워크가 UI를 멈추지 않는다").
    SyncItem {
        from: String,
        reps: Vec<RawRep>,
        summary: String,
        skip_hash: Option<u64>,
    },
    /// ★ 파일 전송 진행(09-12 · DR-30) — 펌프 스레드가 250ms 간격으로 깨운다(패널·배지·트레이 갱신).
    XferTick,
    /// ★ 파일 전송 항목 종결(전부 완료 또는 실패/중지) — 붙여넣기 대기가 있으면 게시·주입.
    XferDone { item_id: u64 },
    /// ★ 섬네일 디코드 완료(09-04 · 30 §4) — 워커 스레드 → 캐시.
    ThumbReady {
        id: u64,
        w: u32,
        h: u32,
        rgba: Vec<u8>,
    },
    /// 디코드 실패 — 진행 중 표시만 푼다.
    ThumbFailed(u64),
    /// ★ 검색 색인 한 건(09-04) — 배경 로더가 blob 본문을 읽어 만든 소문자 검색문.
    SearchText { id: u64, text: String },
    /// ★ 기동 마이그레이션(구본 RGBA → PNG) 한 건 완료 — 셸이 blob으로 기록.
    ThumbEncoded {
        id: u64,
        w: u32,
        h: u32,
        png: Vec<u8>,
    },
    /// 마이그레이션 끝.
    ThumbMigrated,
    /// 트레이 메뉴 "종료".
    Quit,
    /// 감시가 항목을 잡았다 — 게이트를 지나면 이력에 넣는다.
    Captured(Box<ClipSnapshot>),
    /// ★ 최근 항목 선택(T-18e) — 그 항목의 표현 전부를 클립보드로 되돌린다.
    Recent(usize),
    /// ★ 전역 단축키 눌림 — 값 = 동작 id(1 퀵 팝업 · 2 보조 · 3 평문 붙여넣기 · 09-04).
    Hotkey(u32),
    /// ★ 평문 붙여넣기 뒤 원본 클립보드 복원(09-04 — 0.3초 뒤) — 값 = 항목 id.
    RestoreClipboard(u64),
    /// 전역 단축키 등록 결과 — 실패 = 다른 앱이 선점(충돌을 화면에 알린다).
    HotkeyStatus(bool),
    /// ★ 팝업을 닫은 **다음 루프 바퀴**에 주입한다(08-30 Linux 실기 "첫 번만 붙는다").
    ///   winit은 루프로 돌아가야 창 파괴 요청을 flush한다 — 핸들러 안에서 기다리면 팝업이
    ///   아직 떠서 포커스를 쥔 채 `Ctrl+V`를 삼킨다. 값 = 붙여넣기 방식.
    PasteAfterClose(PasteAs),
    /// OS 테마 선호가 바뀌었다(Linux 포털 `SettingChanged`) — `ui.theme = system`이면 따라간다.
    SystemTheme,
    /// ★ 트레이 메뉴 "설정"(09-01) — 메인창 거치지 않고 바로 설정 창.
    OpenSettings,
}

/// 툴팁 문자열 — 보관 수를 함께 보여 준다(감시가 실제로 도는 것이 보인다).
fn tooltip(held: usize) -> String {
    // ★ 프로필 실행(09-04)은 트레이가 둘 — 어느 것인지 툴팁에 표기.
    let name = match crate::conf::profile() {
        Some(p) => format!("Nexa Clip [{p}]"),
        None => "Nexa Clip".to_string(),
    };
    if held == 0 {
        name
    } else {
        format!("{name} — {held}개 보관")
    }
}

/// ★ 좌상단 녹색 점 오버레이(09-03 — beep 화법: 연결됨 배지).
fn overlay_sync_dot(rgba: &mut [u8], side: u32) {
    // ★ 09-03 실기: 아이콘을 가리지 않게 더 작게(5/32) · 좌상단 밀착(중심 = 반지름).
    let r = ((side as i32) * 5 / 32).max(3);
    let (cx, cy) = (r, r);
    for y in 0..side as i32 {
        for x in 0..side as i32 {
            let dx = x - cx;
            let dy = y - cy;
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            let i = ((y * side as i32 + x) * 4) as usize;
            // 테두리(짙은 녹) + 본체(밝은 녹) — 어두운 배경에서도 또렷하게.
            let rim = d2 > (r - 2) * (r - 2);
            let (cr, cg, cb) = if rim {
                (16u8, 96u8, 40u8)
            } else {
                (46u8, 204u8, 64u8)
            };
            rgba[i] = cr;
            rgba[i + 1] = cg;
            rgba[i + 2] = cb;
            rgba[i + 3] = 0xFF;
        }
    }
}

fn content(held: usize, recent: Vec<String>, sync_on: bool) -> TrayContent {
    let lang = current_lang();
    let mut rgba = icon_rgba();
    if sync_on {
        overlay_sync_dot(&mut rgba, ICON_SIDE);
    }
    // ★ 파일 전송 중(09-12) — 좌하단 파랑 점 + 툴팁에 "N개 받는 중 · P%".
    let mut tip = tooltip(held);
    if let Some((n, pct)) = crate::xfer::active_summary() {
        overlay_xfer_dot(&mut rgba, ICON_SIDE);
        let line = tr(lang, Msg::TrayXfer)
            .replacen("{}", &n.to_string(), 1)
            .replacen("{}", &pct.to_string(), 1);
        tip = format!("{tip} — {line}");
    }
    TrayContent {
        rgba,
        side: ICON_SIDE,
        tooltip: tip,
        name: tr(lang, Msg::AppName).to_string(),
        open_label: tr(lang, Msg::TrayOpen).to_string(),
        quit_label: tr(lang, Msg::TrayQuit).to_string(),
        settings_label: tr(lang, Msg::TraySettings).to_string(),
        recent,
    }
}

/// ★ 전송 중 표식(09-12) — 좌하단 파랑 점(상태줄 "None 로컬" 파랑과 같은 계열 · 동기화 녹색 점과 대각).
fn overlay_xfer_dot(rgba: &mut [u8], side: u32) {
    let r = ((side as i32) * 5 / 32).max(3);
    let (cx, cy) = (r, side as i32 - 1 - r);
    for y in 0..side as i32 {
        for x in 0..side as i32 {
            let dx = x - cx;
            let dy = y - cy;
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            let i = ((y * side as i32 + x) * 4) as usize;
            let rim = d2 > (r - 2) * (r - 2);
            let (cr, cg, cb) = if rim {
                (20u8, 60u8, 140u8)
            } else {
                (52u8, 120u8, 246u8)
            };
            rgba[i] = cr;
            rgba[i + 1] = cg;
            rgba[i + 2] = cb;
            rgba[i + 3] = 255;
        }
    }
}

/// ★ 파일 내용 공유 정책(09-12) — 설정 5키 → 바이트 단위 [`crate::xfer::Policy`].
pub(crate) fn xfer_policy(conf: &Settings) -> crate::xfer::Policy {
    let mb = |k: &str, d: u64| conf.state.get(k).parse::<u64>().unwrap_or(d) * 1024 * 1024;
    crate::xfer::Policy {
        contents: conf.state.get("sync.file_contents") == "on",
        auto_bytes: mb("sync.file_auto_mb", 50),
        bg_bps: conf
            .state
            .get("sync.file_bg_kbps")
            .parse::<u64>()
            .unwrap_or(1024)
            * 1024,
        max_bytes: mb("sync.file_max_mb", 1000),
        cache_bytes: mb("sync.file_cache_mb", 2000),
    }
}

/// 이 OS의 붙여넣기 키 표기(알림 문구).
fn paste_key_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘V"
    } else {
        "Ctrl+V"
    }
}

/// 시작마다 자동 시작 등록을 설정값과 동기화한다(멱등 · 외부 삭제 존중).
fn sync_autostart(conf: &mut Settings) {
    // ★ 프로필 실행(09-04)은 시험용 — 로그인 자동 시작 등록을 건드리지 않는다(기본 인스턴스 몫).
    if crate::conf::profile().is_some() {
        return;
    }
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

/// 이력 항목 ↔ 저장 형상 — 필드 1:1(지문만 복원 때 재계산).
fn to_stored(it: &HistoryItem) -> StoredItem {
    StoredItem {
        id: it.id,
        kind: it.kind,
        label: it.label.clone(),
        reps: it.reps.clone(),
        source_app: it.source_app.clone(),
        copies: it.copies,
        pinned: it.pinned,
        // ★ 섬네일 화소는 인덱스에 쓰지 않는다(09-04 · 30 §5 A) — `store_add`가 PNG로 만들어 `thumb_png`에 싣는다.
        thumb: None,
        thumb_ref: it.thumb_ref,
        thumb_png: None,
        created_ms: it.created_ms,
        // ★ 미적재 참조 전달(09-03 지연 로드) — 재기록이 본문 없이도 안전한 근거.
        blobs: it.blob_refs.clone(),
    }
}

fn to_history(it: StoredItem) -> HistoryItem {
    HistoryItem::restored(
        it.id,
        it.kind,
        it.label,
        it.reps,
        it.source_app,
        it.copies,
        it.pinned,
        it.thumb,
        it.thumb_ref,
        it.created_ms,
        it.blobs,
    )
}

/// 상주 셸 — 창·트레이·감시·이력·팝업을 winit 한 루프로.
struct Shell {
    app: App,
    /// ★ 렌더 공용 폰트(09-03 — "이미지로 복사"가 창 없이도 그린다).
    font: nclip_gfx::Font,
    popup: Popup,
    /// ★ S2 메인창(T-18b0) — 항목 관리(핀·삭제·검색·복사). 트레이 좌클릭/"열기"의 목적지.
    main: MainWin,
    tray: TrayHandle,
    /// ★ 이력(T-13) — 팝업·트레이 메뉴와 재적재의 원천. 시작 때 저장소에서 복원된다.
    history: History,
    /// ★ 영속(T-16 · DR-37) — 이력 변화를 이벤트로 흘려보낸다. 열기 실패면 [`NullStore`]
    ///   (이력은 세션 한정으로 돈다 — 안 뜨는 것보다 낫다 · DR-31).
    store: Box<dyn HistoryStore>,
    /// 수집 게이트 — `watch` 진단과 같은 정책(민감·제외 앱·브라우저 암호).
    gate: Gate,
    /// 트레이 메뉴에 보일 최근 개수(`ui.tray_recent_n`).
    tray_n: usize,
    /// ★ K-1 왕복 — 팝업을 열기 **전**의 포그라운드를 기억했다가 되돌린다.
    paste: PlatformPaste,
    /// `paste.auto` — 꺼져 있으면 재적재까지만(주입 없음).
    paste_auto: bool,
    /// ★ 동기화 연결 상태(09-03) — None = 기능 꺼짐 · Some(on) = 켜짐/연결 여부.
    sync_on: Option<bool>,
    /// ★ 원격 항목 에코 차단(09-04) — 방금 적용한 항목의 페이로드 지문·시각. 감시가 우리
    ///   게시를 다시 잡아 승격시켜도 **되돌려 보내지 않는다**(핑퐁 방지).
    sync_skip: Option<(u64, std::time::Instant)>,
    /// 루프에 되돌려 보내는 통로(`PasteAfterClose`).
    proxy: winit::event_loop::EventLoopProxy<ShellEvent>,
    /// ★ 창 레벨(최상위)이 지금 창 백엔드에서 실제로 먹는가 — Wayland 네이티브 창이면
    ///   false(프로토콜에 "항상 위" 요청이 없다). 토글 때 정직 안내에 쓴다(09-02).
    atop_effective: bool,
    /// ★ 마지막 메모리 GC 시각(09-04 · 30 §3).
    last_gc: Instant,
    /// ★ 섬네일 상주 캐시(09-04 · 30 §4) — 메인·팝업과 공유.
    thumbs: crate::thumbs::Thumbs,
    /// 마이그레이션 진행 수(로그용).
    thumb_migrated: usize,
    /// ★ 검색 색인(09-04) — id → 소문자 검색문. 메인·팝업과 공유.
    search_idx: crate::search_index::SearchIndex,
    /// ★ 마지막으로 플랫폼에 넘긴 단축키 설정 서명(09-04) — 바뀔 때만 재등록.
    hotkey_sig: String,
    /// ★ 창 안 키맵 변경 감지(09-08) — 바뀌면 팝업·메인창에 새 표를 준다.
    keymap_sig: String,
    /// ★ 감시 끄기(09-04 사용자 — 툴바 토글): 켜지면 로컬 캡처 사건을 버린다. 세션 한정(재시작 = 켜짐).
    watch_off: bool,
}

/// 캐시된 로컬 경로들을 **모드에 맞는 게시 표현**으로(09-13 · 순수 함수라 테스트가 직접 본다).
///
/// | 모드 | 내보내는 것 |
/// |---|---|
/// | 원본 | 파일 표현 + 경로 텍스트 |
/// | 개체 | 파일 표현만 |
/// | ★ 평문·경로만 | **경로 텍스트만** |
///
/// ⚠️ 마지막 줄이 핵심이다 — 글자만 청했는데 파일 표현을 얹으면 **Linux는 표현을 하나만
/// 게시하므로**([`nclip_plat::clipboard`] `pick_rep`) 파일 표현이 이겨 텍스트가 사라진다.
/// 같은 결함을 [`Shell::resolve_remote_files`]에서 고쳤고(09-13), 여기가 두 번째 자리다.
fn cached_reps(local: &[String], as_: PasteAs) -> Vec<RawRep> {
    let mut reps = if as_.is_text_only() {
        Vec::new()
    } else {
        nclip_plat::clipboard::file_reps(local)
    };
    if as_ != PasteAs::Object {
        reps.extend(nclip_plat::clipboard::plain_text_reps(&local.join("\r\n")));
    }
    reps
}

impl Shell {
    /// 이력이 변했다 — 트레이 메뉴·툴팁을 새 내용으로.
    fn refresh_tray(&self) {
        self.tray.update(content(
            self.history.len(),
            self.history.recent_labels(self.tray_n),
            self.sync_on == Some(true),
        ));
    }

    /// 메인창 닫기 — `ui.close_to_tray` 정책 공유(설정 창과 동일 계약).
    /// ★ blob 지연 로드(09-03) — 미적재 본문을 접근 시점에 복호해 채운다.
    /// 성공 = true. 손상/부재는 채우지 않고 false(있는 척 금지 · DR-31).
    fn ensure_loaded(&mut self, id: u64) -> bool {
        let Some(item) = self.history.get_by_id_mut(id) else {
            return false;
        };
        if item.is_loaded() {
            return true;
        }
        // ★ 참조는 남긴다(09-04 · 30 §3) — 쓰고 나면 다시 내리고, 필요하면 또 읽는다.
        let refs: Vec<(u32, [u8; 32], u64)> = item
            .blob_refs
            .iter()
            .filter(|(ri, _, len)| {
                item.reps
                    .get(*ri as usize)
                    .is_none_or(|r| r.data.len() as u64 != *len)
            })
            .copied()
            .collect();
        let mut fetched = Vec::with_capacity(refs.len());
        for (ri, bid, len) in &refs {
            match self.store.read_blob_by_id(bid) {
                Some(plain) if plain.len() as u64 == *len => fetched.push((*ri, plain)),
                _ => {
                    eprintln!("항목 {id}: blob 복호 실패 — 본문을 채울 수 없음");
                    return false;
                }
            }
        }
        if let Some(item) = self.history.get_by_id_mut(id) {
            for (ri, plain) in fetched {
                if let Some(rep) = item.reps.get_mut(ri as usize) {
                    rep.data = plain;
                }
            }
        }
        true
    }

    /// ★ 쓰고 나면 즉시 내리기(09-04 · DR-42 ②) — 고정 항목은 상주(①). 텍스트류는 남고 이미지 blob만 비운다.
    fn release_body(&mut self, id: u64) {
        if let Some(it) = self.history.get_by_id_mut(id) {
            if !it.pinned {
                it.unload_cold();
            }
        }
    }

    /// ★ 저장소 기록 + blob 참조 회수(09-04 · 30 §3) — 캡처된 항목도 참조를 들고 있어야 나중에 내릴 수 있다.
    /// ★ 섬네일(30 §5 A): RGBA가 있고 참조가 없으면 PNG로 인코드(격리 워커)해 blob으로 — 돌아온 참조를 항목에 붙이고
    ///   RGBA는 캐시로 옮긴 뒤 항목에서 내린다(인덱스·RAM 모두 화소 0).
    fn store_add(&mut self, id: u64) {
        let Some(mut stored) = self.history.get_by_id(id).map(to_stored) else {
            return;
        };
        let rgba = self
            .history
            .get_by_id(id)
            .filter(|it| it.thumb_ref.is_none())
            .and_then(|it| it.thumb.clone());
        if let Some((tw, th, px)) = &rgba {
            match nclip_plat::imgdec::encode_raw_isolated(*tw, *th, px) {
                Some(png) => stored.thumb_png = Some((*tw, *th, png)),
                // 인코드 실패 — 구본 인라인(RGBA)으로라도 남긴다(데이터 우선).
                None => stored.thumb = rgba.clone(),
            }
        }
        let refs = self.store.add(&stored);
        let pinned = self.history.get_by_id(id).is_some_and(|it| it.pinned);
        if let Some(it) = self.history.get_by_id_mut(id) {
            if it.blob_refs.is_empty() && !refs.blobs.is_empty() {
                it.blob_refs = refs.blobs;
            }
            if refs.thumb.is_some() {
                it.thumb_ref = refs.thumb;
                if let Some((tw, th, px)) = it.thumb.take() {
                    self.thumbs.borrow_mut().insert(
                        id,
                        nclip_ctl::theme::IconImage::from_rgba(tw, th, px),
                        pinned,
                    );
                }
            }
        }
    }

    /// ★ 단축키 목록 적용(09-04) — 설정 `key.*`를 읽어 플랫폼에 넘긴다. 바뀐 게 없으면 no-op.
    ///   부팅 1회분은 `run()`이 **트레이 기동 전에** 넘긴다(09-04 mac 실기 — mac·Linux는
    ///   기동 때 한 번만 읽는다) — 여기는 런타임 변경(Windows 즉시 재등록) 몫.
    fn apply_hotkeys(&mut self, force: bool) {
        let (list, label, sig) = hotkey_conf(&self.app.conf);
        if !force && sig == self.hotkey_sig {
            return;
        }
        self.hotkey_sig = sig;
        nclip_plat::tray::set_hotkeys(list, label);
    }

    /// ★ 창 안 키맵 동기(09-08) — 설정 `key.*`(창 안)가 바뀌면 팝업·메인창에 새 표를 준다. 바뀐 게 없으면 no-op.
    fn apply_keymap(&mut self) {
        let sig = crate::keys::Keymap::sig(&self.app.conf);
        if sig == self.keymap_sig {
            return;
        }
        self.keymap_sig = sig;
        let km = crate::keys::Keymap::from_conf(&self.app.conf);
        self.main.set_keymap(km.clone());
        self.popup.set_keymap(km);
    }

    /// ★ 평문 붙여넣기 단축키(09-04 · CopyQ "클립보드를 일반 문자로 붙여넣기"): 맨 앞 항목의 평문을 게시 → 지금 포그라운드에
    ///   Ctrl+V 주입 → 0.3초 뒤 원본 표현으로 복원(사용자 결정 — 이후 일반 붙여넣기는 서식 그대로).
    fn paste_plain_hotkey(&mut self) {
        let Some(id) = self.history.get(0).map(|it| it.id) else {
            eprintln!("평문 붙여넣기: 이력이 비어 있습니다");
            return;
        };
        let _ = self.ensure_loaded(id);
        let Some(item) = self.history.get_by_id(id) else {
            return;
        };
        let reps = Self::reps_for_mode(&item.reps, PasteAs::Plain);
        if reps.is_empty() {
            eprintln!("평문 붙여넣기: 맨 앞 항목에 평문 표현이 없습니다");
            return;
        }
        if !self.paste.capture_focus() {
            eprintln!("평문 붙여넣기: 대상 창을 기억하지 못했습니다");
        }
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(_) => self.history.expect_echo(0),
            Err(e) => {
                eprintln!("평문 붙여넣기: 게시 실패 — {e}");
                return;
            }
        }
        match self.paste.restore_and_paste(PasteAs::Plain) {
            Ok(()) => println!("평문 붙여넣기: 주입 ok — 0.3초 뒤 원본 복원"),
            Err(e) => {
                eprintln!("평문 붙여넣기: 주입 실패 {e:?} — 클립보드에는 평문이 실려 있습니다")
            }
        }
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let _ = proxy.send_event(ShellEvent::RestoreClipboard(id));
        });
    }

    /// 원본 복원 — 그 항목의 표현 전부를 다시 게시(에코는 승격으로 흡수).
    fn restore_clipboard(&mut self, id: u64) {
        let _ = self.ensure_loaded(id);
        let Some(pos) =
            (0..self.history.len()).find(|&i| self.history.get(i).is_some_and(|it| it.id == id))
        else {
            return;
        };
        let Some(item) = self.history.get(pos) else {
            return;
        };
        let reps = item.reps.clone();
        if reps.iter().all(|r| r.data.is_empty()) {
            return;
        }
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!("평문 붙여넣기: 원본 복원 — 표현 {n}개");
                self.history.expect_echo(pos);
            }
            Err(e) => eprintln!("원본 복원 실패: {e}"),
        }
        self.release_body(id);
    }

    /// ★ 설정 창 열기(09-04 사용자) — 메인창 기하·최상위 여부를 앵커로 넘긴다(메인창 모니터·옆 배치 · 위에 뜨게).
    fn open_settings(&mut self, el: &ActiveEventLoop) {
        let on_top =
            self.app.conf.state.get("ui.always_on_top") == "on" && self.main.window_id().is_some();
        self.app.set_anchor(self.main.geometry(), on_top);
        self.app.ensure_window(el);
    }

    /// ★ 검색 방식 선택(09-04 드롭다운) — 설정에 쓰면 박동 동기가 두 창의 방식·드롭다운·필터를 맞춘다.
    fn set_find_mode(&mut self, v: &str) {
        println!(
            "검색 방식 선택: {v} (현재 {})",
            self.app.conf.state.get("find.mode")
        );
        if self.app.conf.state.get("find.mode") != v {
            self.app
                .conf
                .set("find.mode", v.to_string(), Instant::now());
            println!("검색 방식: {v}");
        }
    }

    /// ★ 검색문 갱신(09-04 색인) — 손에 있는 본문(inline 또는 적재된 blob)으로. 본문이 미적재 blob이면 라벨만(배경 로더가 채운다).
    fn index_item(&mut self, id: u64) {
        let Some(it) = self.history.get_by_id(id) else {
            return;
        };
        let plain =
            crate::main_win::plain_of(&it.reps).or_else(|| nclip_core::capture::svg_text(&it.reps));
        let text = nclip_core::search::search_text(
            &it.label,
            plain.as_deref(),
            crate::search_index::TEXT_CAP,
        );
        self.search_idx.borrow_mut().insert(id, Rc::from(text));
    }

    /// 맨 앞 항목(방금 push된 것) 색인.
    fn index_front(&mut self) {
        if let Some(id) = self.history.get(0).map(|it| it.id) {
            self.index_item(id);
        }
    }

    /// ★ 기동 색인(09-04): inline 본문은 즉시, blob 본문(텍스트 계열 · 미적재)은 **배경 스레드**가 한 번 읽어 채운다.
    fn build_search_index(&mut self) {
        let ids: Vec<u64> = (0..self.history.len())
            .filter_map(|i| self.history.get(i).map(|it| it.id))
            .collect();
        let mut pending: Vec<PendingText> = Vec::new();
        for id in ids {
            self.index_item(id);
            let Some(it) = self.history.get_by_id(id) else {
                continue;
            };
            // 텍스트 계열인데 본문이 비어 있는(미적재 blob) 표현 — 평문 우선순위대로.
            let mut refs: Vec<(u8, String, [u8; 32], u64)> = it
                .blob_refs
                .iter()
                .filter_map(|(ri, bid, len)| {
                    let r = it.reps.get(*ri as usize)?;
                    if !r.data.is_empty() {
                        return None;
                    }
                    let rank = nclip_core::capture::plain_rank(&r.format)?;
                    Some((rank, r.format.clone(), *bid, *len))
                })
                .collect();
            if refs.is_empty() {
                continue;
            }
            refs.sort_by_key(|r| r.0);
            pending.push((
                id,
                it.label.clone(),
                refs.into_iter().map(|(_, f, b, l)| (f, b, l)).collect(),
            ));
        }
        if pending.is_empty() {
            return;
        }
        let Some(reader) = self.store.blob_reader() else {
            return;
        };
        println!(
            "검색 색인: 본문 blob {}건을 배경에서 읽습니다",
            pending.len()
        );
        let proxy = self.proxy.clone();
        std::thread::Builder::new()
            .name("search-index".into())
            .spawn(move || {
                for (id, label, refs) in pending {
                    let mut plain = None;
                    for (fmt, bid, len) in refs {
                        if let Some(data) = reader.read(&bid) {
                            if data.len() as u64 == len {
                                if let Some(t) = nclip_core::capture::decode_plain(&fmt, &data) {
                                    plain = Some(t);
                                    break;
                                }
                            }
                        }
                    }
                    let text = nclip_core::search::search_text(
                        &label,
                        plain.as_deref(),
                        crate::search_index::TEXT_CAP,
                    );
                    let _ = proxy.send_event(ShellEvent::SearchText { id, text });
                }
            })
            .ok();
    }

    /// ★ 기동 마이그레이션(09-04 · 30 §5 A): 구본 인라인 RGBA 섬네일을 항목에서 **바로 떼어**(RSS 즉시 ↓) 워커 스레드가
    ///   384²로 줄여 PNG로 인코드 → `ThumbEncoded`로 돌아오면 blob으로 기록. 오래된 것부터(재생 순서 보존).
    fn start_thumb_migration(&mut self) {
        let mut legacy: Vec<(u64, u32, u32, Vec<u8>)> = Vec::new();
        for i in (0..self.history.len()).rev() {
            let Some(id) = self.history.get(i).map(|it| it.id) else {
                continue;
            };
            if let Some(it) = self.history.get_by_id_mut(id) {
                if it.thumb_ref.is_none() {
                    if let Some((w, h, px)) = it.thumb.take() {
                        legacy.push((id, w, h, px));
                    }
                }
            }
        }
        if legacy.is_empty() {
            return;
        }
        println!(
            "섬네일 마이그레이션: {}개 — RGBA 인덱스 → PNG blob(384²) · 배경",
            legacy.len()
        );
        let proxy = self.proxy.clone();
        std::thread::Builder::new()
            .name("thumb-migrate".into())
            .spawn(move || {
                for (id, w, h, px) in legacy {
                    let (w, h, px) = nclip_core::img::downscale_rgba(w, h, &px, THUMB_SIDE)
                        .unwrap_or((w, h, px));
                    if let Some(png) = nclip_plat::imgdec::encode_raw_isolated(w, h, &px) {
                        let _ = proxy.send_event(ShellEvent::ThumbEncoded { id, w, h, png });
                    }
                    // 워커 프로세스 폭주 방지 — 항목당 잠깐 쉰다(총 수 초).
                    std::thread::sleep(Duration::from_millis(5));
                }
                let _ = proxy.send_event(ShellEvent::ThumbMigrated);
            })
            .ok();
    }

    /// 마이그레이션 결과 — 참조 없는 항목만(그새 재복사돼 새 PNG가 생겼으면 버린다).
    fn on_thumb_encoded(&mut self, id: u64, w: u32, h: u32, png: Vec<u8>) {
        let Some(mut stored) = self
            .history
            .get_by_id(id)
            .filter(|it| it.thumb_ref.is_none())
            .map(to_stored)
        else {
            return;
        };
        stored.thumb_png = Some((w, h, png));
        let refs = self.store.add(&stored);
        if let Some(it) = self.history.get_by_id_mut(id) {
            if it.blob_refs.is_empty() && !refs.blobs.is_empty() {
                it.blob_refs = refs.blobs;
            }
            it.thumb_ref = refs.thumb;
        }
        self.thumb_migrated += 1;
    }

    /// ★ 뷰포트 섬네일 펌프(09-04 · 30 §4): 그리기 루프가 남긴 요청을 최대 4개 꺼내 워커 스레드에 디코드를 맡긴다.
    ///   RGBA가 아직 항목에 있으면(막 캡처·승격) 바로 캐시에 넣는다.
    fn pump_thumbs(&mut self) {
        let wanted = self.thumbs.borrow_mut().take_wanted(4);
        for id in wanted {
            let Some(it) = self.history.get_by_id(id) else {
                self.thumbs.borrow_mut().fail(id);
                continue;
            };
            let pinned = it.pinned;
            if let Some((w, h, px)) = &it.thumb {
                let img = nclip_ctl::theme::IconImage::from_rgba(*w, *h, px.clone());
                self.thumbs.borrow_mut().insert(id, img, pinned);
                self.main.redraw_now();
                self.popup.redraw_now();
                continue;
            }
            let Some((bid, len, _, _)) = it.thumb_ref else {
                // ★ 자가 치유(09-05 mac 실기 — 옛 워커가 팔레트 PNG를 못 읽어 섬네일 없이 저장된 항목):
                //   참조가 없으면 본문 표현에서 섬네일을 만들어 표시하고, PNG blob으로 영속한다
                //   (`on_thumb_encoded` = 마이그레이션 경로 재사용). 이미지 계열이 아니면 종전대로 실패.
                let is_image = matches!(
                    it.kind,
                    nclip_core::ClipKind::Image | nclip_core::ClipKind::Object
                );
                if !is_image {
                    self.thumbs.borrow_mut().fail(id);
                    continue;
                }
                let _ = self.ensure_loaded(id);
                let reps: Vec<RawRep> = self
                    .history
                    .get_by_id(id)
                    .map(|it| it.reps.clone())
                    .unwrap_or_default();
                if reps.iter().all(|r| r.data.is_empty()) {
                    self.thumbs.borrow_mut().fail(id);
                    continue;
                }
                let proxy = self.proxy.clone();
                std::thread::Builder::new()
                    .name("thumb-rebuild".into())
                    .spawn(move || match decode_image(&reps, THUMB_SIDE) {
                        Some((w, h, rgba)) => {
                            if let Some(png) = nclip_plat::imgdec::encode_raw_isolated(w, h, &rgba)
                            {
                                let _ =
                                    proxy.send_event(ShellEvent::ThumbEncoded { id, w, h, png });
                            }
                            let _ = proxy.send_event(ShellEvent::ThumbReady { id, w, h, rgba });
                        }
                        None => {
                            let _ = proxy.send_event(ShellEvent::ThumbFailed(id));
                        }
                    })
                    .ok();
                continue;
            };
            let Some(png) = self.store.read_blob_by_id(&bid) else {
                self.thumbs.borrow_mut().fail(id);
                continue;
            };
            if png.len() as u64 != len {
                self.thumbs.borrow_mut().fail(id);
                continue;
            }
            let proxy = self.proxy.clone();
            std::thread::Builder::new()
                .name("thumb-decode".into())
                .spawn(move || {
                    let ev = match nclip_plat::imgdec::decode_isolated(&png, THUMB_SIDE) {
                        Some((w, h, rgba)) => ShellEvent::ThumbReady { id, w, h, rgba },
                        None => ShellEvent::ThumbFailed(id),
                    };
                    let _ = proxy.send_event(ev);
                })
                .ok();
        }
    }

    /// ★ 메모리 GC(09-04 · 30 §3 · DR-42): 30초마다, 또는 상주가 활동 상한(96MB)을 넘으면 즉시 —
    ///   고정·미리보기 중 항목을 뺀 냉 본문을 내린다. 드롭뿐이라 UI를 막지 않는다.
    fn gc_memory(&mut self, force: bool) {
        const ACTIVE_CAP: u64 = 96 << 20;
        let resident = self.history.resident_bytes();
        if !force && resident <= ACTIVE_CAP && self.last_gc.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_gc = Instant::now();
        let keep = self.main.preview_id();
        let freed = self.history.unload_cold_except(keep);
        // 섬네일 캐시 — 상한 초과면 8장까지 줄인다(고정 제외).
        let thumb_bytes = {
            let mut t = self.thumbs.borrow_mut();
            if resident > ACTIVE_CAP {
                t.trim(8);
            }
            t.bytes()
        };
        if freed > 0 {
            println!(
                "메모리 GC: 본문 {}MB 내림 → 상주 {}MB + 섬네일 캐시 {}MB",
                freed >> 20,
                self.history.resident_bytes() >> 20,
                thumb_bytes >> 20
            );
        }
    }

    /// ★ K4 미리보기 펌프 — 메인창이 원본 이미지를 원하면 지연 디코드해 넘긴다.
    fn pump_preview(&mut self) {
        if let Some(pid) = self.main.take_preview_request() {
            let _ = self.ensure_loaded(pid); // 이미지 본문(blob) 지연 로드(09-03).
            let img = self
                .history
                .get_by_id(pid)
                .and_then(|it| decode_image(&it.reps, PREVIEW_SIDE));
            match img {
                Some((w, h, rgba)) => self.main.set_preview_image(pid, w, h, rgba),
                // 디코드 실패(이미지 표현 없는 Object 포함) — 텍스트 폴백으로 전환.
                None => self.main.set_preview_failed(pid),
            }
            // 디코드본은 미리보기가 따로 든다 — 원본 바이트는 내린다(DR-42 ②).
            self.release_body(pid);
        }
    }

    fn close_main(&mut self) {
        // 메인창 X/Esc = 항상 숨김(상주 유지 — 종료는 트레이 Quit만 · 08-30 확정 계승).
        self.save_main_geom();
        self.main.close();
    }

    /// ★ 메인창 기하 저장(09-01 사용자 요청 "닫힌 위치에 다시") — `ui.win_*` 키(예약되어 있던 자리).
    fn save_main_geom(&mut self) {
        if let Some((x, y, w, h)) = self.main.geometry() {
            let now = Instant::now();
            self.app.conf.set("ui.win_x", x.to_string(), now);
            self.app.conf.set("ui.win_y", y.to_string(), now);
            self.app.conf.set("ui.win_w", w.to_string(), now);
            self.app.conf.set("ui.win_h", h.to_string(), now);
        }
    }

    fn saved_main_geom(&self) -> Option<(i32, i32, u32, u32)> {
        let gi = |k: &str| self.app.conf.state.get(k).parse::<i32>().ok();
        let gu = |k: &str| self.app.conf.state.get(k).parse::<u32>().ok();
        Some((
            gi("ui.win_x")?,
            gi("ui.win_y")?,
            gu("ui.win_w")?,
            gu("ui.win_h")?,
        ))
    }

    /// ★ 메인창 복사 — 재적재만(주입 없음 · 관리 화면). 에코는 승격으로.
    /// ★ 편집 저장(S4 평문화 · 09-01 확정) — 같은 id로 평문 교체 + 저장소 Add(재생 교체).
    fn save_edit(&mut self, id: u64, text: &str) {
        let _ = self.ensure_loaded(id);
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            eprintln!("빈 내용은 저장하지 않습니다 — 삭제를 쓰세요");
            return;
        }
        let line = trimmed.lines().next().unwrap_or_default();
        let label = clip_text(line, MENU_LABEL_CHARS);
        let reps = nclip_plat::clipboard::plain_text_reps(trimmed);
        if self
            .history
            .replace_content(id, nclip_core::ClipKind::Text, label, reps)
        {
            if let Some(it) = self.history.get_by_id(id) {
                let __sid = it.id;
                self.store_add(__sid);
                self.index_item(__sid);
            }
            self.main.on_history_changed(&self.history);
            self.refresh_tray();
            println!("편집 저장: 항목 {id} — 평문으로 교체");
        }
    }

    /// ★ 모드 선별 + 폴백(09-01 J2) — 파일 항목의 "경로만"은 평문 표현이 아예 없을 수
    /// 있어(탐색기 복사 = CF_HDROP뿐) 경로 목록에서 평문을 **만들어** 준다.
    ///
    /// ★ 표현만 받는 **순수 함수**다(09-13) — 항목 전체가 아니라 `reps`만 보므로
    /// 테스트가 `HistoryItem` 조립 없이 3-OS 표현을 그대로 넣어 볼 수 있다.
    fn reps_for_mode(reps: &[RawRep], as_: PasteAs) -> Vec<RawRep> {
        let filtered = as_.filter_reps(reps);
        if !filtered.is_empty() {
            return filtered;
        }
        match as_ {
            // ★ 경로만·평문 — 평문 표현이 없는 파일 항목은 **경로 목록에서 합성**한다.
            //   ⚠️ 09-13 수정: 종전에는 `CF_HDROP`에서만 뽑아 **Windows 전용**이었다 —
            //   Linux(`x-special/gnome-copied-files`)·mac(`NSFilenamesPboardType`)에서는
            //   빈 표현이 나와 아무 일도 일어나지 않았다(사용자 실기 09-13).
            //   `file_paths()`(= `capture::paths_of`)는 네 OS 표현을 이미 전부 안다.
            PasteAs::PathOnly | PasteAs::Plain => {
                let paths = nclip_core::capture::paths_of(reps);
                if paths.is_empty() {
                    // 평문은 파일이 아닌 항목도 온다 — PPT 글상자는 SVG에서 글자를 뽑는다
                    //   (09-02 사용자 요청 · 표시와 같은 `<text>` 추출).
                    return nclip_core::capture::svg_text(reps)
                        .map(|t| nclip_plat::clipboard::plain_text_reps(&t))
                        .unwrap_or_default();
                }
                nclip_plat::clipboard::plain_text_reps(&paths.join("\r\n"))
            }
            _ => Vec::new(),
        }
    }

    fn copy_from_main(&mut self, id: u64, as_: PasteAs) {
        let _ = self.ensure_loaded(id); // 본문 지연 로드(09-03).
        let Some(pos) =
            (0..self.history.len()).find(|&i| self.history.get(i).is_some_and(|it| it.id == id))
        else {
            return;
        };
        let Some(item) = self.history.get(pos) else {
            return;
        };
        let (item_reps, label) = (item.reps.clone(), item.label.clone());
        let by_mode = Self::reps_for_mode(&item.reps, as_);
        let reps: Vec<RawRep> = match self.resolve_remote_files(id, &item_reps, as_) {
            Some(Resolved::Ready(r) | Resolved::Text(r)) => r,
            Some(Resolved::Fetching) => return,
            None => by_mode,
        };
        if reps.is_empty() {
            eprintln!("평문 표현이 없는 항목입니다");
            return;
        }
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!("복사(메인창): \"{label}\" — 표현 {n}개");
                self.history.expect_echo(pos);
            }
            Err(e) => eprintln!("복사 실패: {e}"),
        }
        self.release_body(id);
    }

    /// 항목을 클립보드로 되돌린다(트레이 메뉴 — 주입 없음).
    fn repost(&mut self, i: usize) {
        let Some(item) = self.history.get(i) else {
            return;
        };
        let (id, item_reps, label) = (item.id, item.reps.clone(), item.label.clone());
        let reps = match self.resolve_remote_files(id, &item_reps, PasteAs::Original) {
            Some(Resolved::Ready(r) | Resolved::Text(r)) => r,
            Some(Resolved::Fetching) => return,
            None => item_reps,
        };
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!("재적재: \"{label}\" — 표현 {n}개 게시");
                // ★ 부분 게시의 에코는 원본 승격으로(08-30 Linux 실기 "같은 항목 둘").
                self.history.expect_echo(i);
            }
            Err(e) => eprintln!("재적재 실패: {e}"),
        }
    }

    /// ★ 연결 표시 재판정(09-04) — None = 기능 꺼짐(아이콘 없음) · Some(on) = 릴레이 연결 ∨ LAN 피어 연결.
    fn refresh_sync_indicator(&mut self) {
        use crate::main_win::SyncMode;
        use crate::sync_cmd::SyncStatus as S;
        // ★ 09-04 사용자: 트레이 배지·툴바 아이콘은 **릴레이 연결** 기준(None = 네트워크 미연결 상태) ·
        //   상태줄 점은 모드별 색(녹 릴레이 · 파랑 None · 진회색 미사용 · 흐림 릴레이 미연결).
        let mode = match crate::sync_cmd::status() {
            S::Off => SyncMode::Off,
            S::LanOnly => SyncMode::Local,
            S::Connected => SyncMode::Relay,
            S::Connecting | S::Failed(_) | S::Stopped | S::Unconfigured => SyncMode::RelayDown,
        };
        let next = match mode {
            SyncMode::Off => None,
            SyncMode::Relay => Some(true),
            SyncMode::Local | SyncMode::RelayDown => Some(false),
        };
        self.main.set_sync_mode(mode);
        if self.sync_on != next {
            self.sync_on = next;
            self.refresh_tray();
            self.main.set_sync_state(self.sync_on);
            println!(
                "동기화 상태: {}",
                match mode {
                    SyncMode::Relay => "릴레이 연결됨",
                    SyncMode::Local => "로컬(None) — 릴레이 미연결",
                    SyncMode::RelayDown => "릴레이 끊김",
                    SyncMode::Off => "꺼짐",
                }
            );
        }
    }

    /// 캡처(감시) 또는 원격 항목(`remote = Some(기기명)`)을 이력에 넣는다 — 게이트·요약·썸네일·
    /// 영속·화면 갱신은 공용. ★ 우리 복사만 다른 기기로 전파한다(09-04 · DR-6).
    fn on_captured(&mut self, mut snap: Box<ClipSnapshot>, remote: Option<&str>) {
        // ★ 감시 끄기(09-04) — 로컬 캡처만 버린다(수신 항목은 감시가 아니라 동기화).
        if self.watch_off && remote.is_none() {
            return;
        }
        // ★ CF_HTML 정제(T-14d · D-62 1단) — 캡처 때 한 번만(재적재·저장은 이미 깨끗).
        for r in &mut snap.reps {
            if r.format == "HTML Format" {
                if let Some(clean) = nclip_core::capture::sanitize_cf_html(&r.data) {
                    println!(
                        "HTML 정제: {}B → {}B (script/이벤트 속성 제거)",
                        r.data.len(),
                        clean.len()
                    );
                    r.data = clean;
                }
            } else if nclip_core::capture::is_html_format(&r.format) {
                // ★ mac `public.html`·Linux `text/html`(09-04) — 헤더 없는 HTML도 같은 경계를 지난다.
                if let Some(clean) = nclip_core::capture::sanitize_plain_html(&r.data) {
                    println!(
                        "HTML 정제({}): {}B → {}B",
                        r.format,
                        r.data.len(),
                        clean.len()
                    );
                    r.data = clean;
                }
            }
        }
        // ★ 설정 즉시 반영 — 설정 창에서 바꾼 값이 다음 캡처부터 산다
        //   (게이트·상한·메뉴 개수·자동 붙여넣기 — 재시작 불요).
        self.gate = Gate::from_state(&self.app.conf);
        self.history.set_cap(
            self.app
                .conf
                .state
                .get("store.max_items")
                .parse()
                .unwrap_or(1000),
        );
        // ★ 보관 예산(T-13 · 09-01 확정: 기본 기간 무제한 + 500MB).
        let mb: u64 = self
            .app
            .conf
            .state
            .get("store.max_total_mb")
            .parse()
            .unwrap_or(500);
        let days: u64 = self
            .app
            .conf
            .state
            .get("store.max_age_days")
            .parse()
            .unwrap_or(0);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.history
            .set_budget(mb * 1_000_000, days * 86_400_000, now_ms);
        self.tray_n = self
            .app
            .conf
            .state
            .get("ui.tray_recent_n")
            .parse()
            .unwrap_or(8);
        self.paste_auto = self.app.conf.state.get("paste.auto") == "on";
        // 게이트 — `watch`와 같은 정책. 막힌 것은 이력에도 안 들어간다.
        if self.gate.blocks(&snap).is_some() {
            return;
        }
        let (kind, line) = summarize(&snap);
        let label = clip_text(&line, MENU_LABEL_CHARS);
        // ★ 이미지 썸네일(08-28 사용자 요청) — 설정이 켜졌을 때만 만든다.
        let thumb = (matches!(
            kind,
            nclip_core::ClipKind::Image | nclip_core::ClipKind::Object
        ) && self.app.conf.state.get("ui.image_preview") == "on")
            .then(|| make_thumb(&snap.reps))
            .flatten();
        // ★ 캐시 에코(09-12 · 2PC 연쇄 차단): 원격 파일 약속을 캐시에서 게시하면 감시가 그 `CF_HDROP`을
        //   되읽는다. 표현이 약속(매니페스트)과 달라 지문·부분집합 규칙으로는 못 알아보고 **새 로컬 파일
        //   항목**이 되어 상대에게 전파됐다 → 상대가 우리 캐시 경로를 약속으로 받아 또 캐시·게시 → 무한.
        //   여기서 경로가 전부 우리 캐시 안이면 **그 약속 항목의 승격**으로 흡수하고 전파하지 않는다.
        let own_cache = kind == nclip_core::ClipKind::Files
            && remote.is_none()
            && crate::xfer::all_cache_paths(&nclip_core::paths_of(&snap.reps));
        if own_cache {
            let paths = nclip_core::paths_of(&snap.reps);
            let mut i = 0usize;
            let mut hit = None;
            while let Some(it) = self.history.get(i) {
                i += 1;
                let Some(m) = nclip_core::RemoteFiles::of_reps(&it.reps) else {
                    continue;
                };
                let cached = crate::xfer::with(|x| x.all_cached(&m)).flatten();
                if cached.is_some_and(|c| {
                    c.len() == paths.len()
                        && c.iter()
                            .zip(&paths)
                            .all(|(a, b)| a.to_string_lossy().eq_ignore_ascii_case(b))
                }) {
                    hit = Some(it.id);
                    break;
                }
            }
            match hit {
                Some(id) if self.history.promote(id) => {
                    self.store.touch(id);
                    println!("이력: Promoted — 캐시 게시 에코(원격 파일 약속 승격 · 전파 안 함)");
                }
                _ => println!("이력: 캐시 경로 캡처 — 약속 항목 없음(무시 · 전파 안 함)"),
            }
            self.index_front();
            self.refresh_tray();
            self.main.on_history_changed(&self.history);
            if self.popup.is_open() {
                self.popup.on_history_changed(&self.history);
            }
            return;
        }
        // ★ 재적재로 되돌아온 우리 게시도 여기로 온다 — 승격(맨 위로)이
        //   곧 에코 처리다(항목이 늘지 않는다).
        let pushed: Pushed = self.history.push(&snap, kind, label, thumb);
        self.index_front();
        // ★ push 판정 가시화(09-04 mac 실기 "더블클릭 승격 안 됨" 진단) — 에코가
        //   승격(Promoted)으로 흡수됐는지 새 항목(New)으로 늘었는지 로그로 판정한다.
        if let Some(front) = self.history.get(0) {
            println!(
                "이력: {:?} — \"{}\" (표현 {}개)",
                pushed,
                front.label,
                snap.reps.len()
            );
        }
        // ★ 파일 내용 공유(09-12 · DR-30) — 내 파일 항목은 서빙 가능 경로로 제안하고,
        //   받은 약속은 상한 이하면 백그라운드 사전 캐시 줄에 세운다(저속 · Fg에 양보).
        if kind == nclip_core::ClipKind::Files {
            if let Some(front) = self.history.get(0) {
                match nclip_core::RemoteFiles::of_reps(&front.reps) {
                    Some(m) if remote.is_some() => {
                        let id = front.id;
                        if let Some(crate::xfer::Fetch::Started(n)) =
                            crate::xfer::with(|x| x.fetch(id, &m, crate::xfer::Prio::Bg))
                        {
                            if n > 0 {
                                // ★ 최신 항목은 제한 없이 바로 받는다(09-13 사용자 — "복사하면 바로 붙여넣기") —
                                //   끝났을 때 아직 이력 맨 앞이면 `on_xfer_done`이 클립보드에 게시한다.
                                let fg = crate::xfer::with(|x| x.boost(id)).unwrap_or(0);
                                println!(
                                    "파일 공유: {}의 파일 {n}개({}MB) 즉시 받기(Fg {fg} · 저속 상한 없음) — 완료 시 최신 항목이면 클립보드 반영",
                                    m.origin_name,
                                    m.total_bytes() / 1_048_576
                                );
                            }
                        }
                    }
                    None => crate::xfer::offer(&nclip_core::paths_of(&front.reps)),
                    Some(_) => {}
                }
            }
        }
        // ★ 영속(T-16) — 이력 변화를 그대로 이벤트로 흘린다(id가 짝이다).
        match pushed {
            Pushed::New | Pushed::Replaced => {
                if let Some(front) = self.history.get(0) {
                    let __sid = front.id;
                    self.store_add(__sid);
                }
            }
            Pushed::Promoted => {
                if let Some(front) = self.history.get(0) {
                    // ★ 승격이 램 섬네일을 채웠을 수 있다(무섬네일 세대 항목 재복사 —
                    //   09-02 실기: TOUCH만 남기면 재시작 후 텍스트로 퇴행). 섬네일이
                    //   있으면 ADD로 온전히 재기록 — 블롭은 내용 주소라 비용 미미.
                    if front.thumb.is_some() {
                        let __sid = front.id;
                        self.store_add(__sid);
                    } else {
                        self.store.touch(front.id);
                    }
                }
            }
        }
        for id in self.history.drain_evicted() {
            self.store.remove(id);
        }
        self.refresh_tray();
        self.main.on_history_changed(&self.history);
        if self.popup.is_open() {
            self.popup.on_history_changed(&self.history);
        }
        // ★ 재전송 금지(09-04 사용자) — 방금 push된 항목(맨 앞)이 원격 수신 표식(⇄)을
        //   달고 있으면 에코·재복사·승격 어느 경로든 되돌려 보내지 않는다(핑퐁 원천 차단 ·
        //   페이로드 지문 10초 가드는 보조로 유지).
        let front_remote = self.history.get(0).is_some_and(|it| {
            it.source_app
                .as_deref()
                .is_some_and(|s| s.starts_with(REMOTE_MARK))
        });
        if remote.is_none() && !front_remote {
            self.maybe_broadcast(&snap);
        }
    }

    /// ★ 전파(DR-6) — 릴레이에 붙어 있고 승인된 기기가 있을 때, 휴대 페이로드로 보낸다.
    ///   방금 원격에서 받아 게시한 항목의 에코(같은 페이로드 지문)는 보내지 않는다.
    fn maybe_broadcast(&mut self, snap: &ClipSnapshot) {
        // 릴레이 연결 여부로 막지 않는다(09-04 LAN 직결) — 승인·온라인 피어가 없으면 broadcast가 0을 돌려준다.
        if !crate::sync_cmd::has_peers() {
            return; // 보낼 곳이 없으면 워커도 띄우지 않는다(대부분의 복사).
        }
        // ★ 우리 캐시의 파일(원격 약속을 실체화한 것)은 **절대 전파하지 않는다**(09-12 연쇄 차단 · 이중 방어).
        if crate::xfer::all_cache_paths(&nclip_core::paths_of(&snap.reps)) {
            return;
        }
        // ★ 페이로드 생성(DIB→PNG 인코드 등)은 **워커 스레드**에서 — UI 스레드는 표현 복제만(09-04 사용자:
        //   "네트워크 처리가 프로그램을 멈추거나 지연시키지 않게").
        let reps = snap.reps.clone();
        let skip = self.sync_skip;
        // ★ 파일 경로 전파 정책(09-12)은 **여기(UI 스레드)**서 읽는다 — 설정 맵 조회는 값싸고,
        //   워커에 값으로 넘기면 설정 창에서 방금 바꾼 값이 다음 복사부터 바로 산다.
        //   `None` = 파일 항목을 보내지 않는다(`sync.files` 끔).
        let files_max = (self.app.conf.state.get("sync.files") == "on").then(|| {
            self.app
                .conf
                .state
                .get("sync.files_max")
                .parse::<usize>()
                .unwrap_or(1000)
                .max(1)
        });
        let _ = std::thread::Builder::new()
            .name("nclip-sync-out".into())
            .spawn(move || {
                let Some(payload) = crate::syncitem::from_reps_limited(&reps, files_max) else {
                    return;
                };
                let h = crate::syncitem::hash(&payload);
                if let Some((sh, t)) = skip {
                    if sh == h && t.elapsed() < std::time::Duration::from_secs(10) {
                        return; // 원격 항목의 에코 — 되돌려 보내지 않는다.
                    }
                }
                let n = crate::sync_cmd::broadcast(payload);
                if n > 0 {
                    println!("동기화: 항목 전파 → 기기 {n}대");
                }
            });
    }

    /// ★ 원격 항목 적용(09-04) — 이력 등재(출처 = 기기명) + **클립보드 게시**(다른 기기에서
    ///   바로 Ctrl+V) + 에코 흡수(감시가 다시 잡으면 승격 · 되돌려 보내지 않음).
    fn apply_remote(
        &mut self,
        from: &str,
        reps: Vec<RawRep>,
        summary: &str,
        skip_hash: Option<u64>,
    ) {
        if reps.is_empty() {
            return;
        }
        // 에코 지문(세션 스레드가 **우리가 게시할 표현**으로 계산) — 감시가 다시 읽어 오는 것이 이것이다.
        if let Some(h) = skip_hash {
            self.sync_skip = Some((h, std::time::Instant::now()));
        }
        println!("동기화: ← {from} 항목 수신 — {summary}");
        let promise = crate::syncitem::has_promise(&reps);
        let snap = ClipSnapshot {
            reps: reps.clone(),
            source_app: Some(format!("{REMOTE_MARK}{from}")),
            concealed: false,
            seq: 0,
        };
        self.on_captured(Box::new(snap), Some(from));
        if promise {
            // ★ 파일 약속(DR-30) — 지금 게시하지 않는다(붙여넣을 때 받는다). 사전 캐시는 on_captured가 줄 세웠다.
            println!("동기화: 파일 약속 등재 — 목록에서 고르면 {from}에서 받아 붙여넣습니다");
            return;
        }
        // ★ 클립보드 열기 경합(09-04 실기 — 다른 앱/인스턴스가 쥔 순간) — 짧게 몇 번 다시 시도.
        let mut last = Err(String::new());
        for attempt in 0..5 {
            last = nclip_plat::clipboard::set_reps(&reps);
            if last.is_ok() {
                break;
            }
            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
        match last {
            Ok(n) => {
                println!("동기화: 클립보드 게시 — 표현 {n}개(이 PC에서 바로 붙여넣기 가능)");
                self.history.expect_echo(0);
            }
            Err(e) => eprintln!("동기화: 클립보드 게시 실패({e}) — 이력에는 들어갔습니다"),
        }
    }

    /// ★ 팝업 선택 — 재적재 후 **기억해 둔 창으로 복원 + `Ctrl+V` 주입**(K-1 실물 경로).
    /// 4모드(T-15b)는 **클립보드 내용 선별**로 구현한다 — 주입 키는 항상 Ctrl+V.
    fn pick(&mut self, index: usize, as_: PasteAs) {
        if let Some(id) = self.history.get(index).map(|it| it.id) {
            let _ = self.ensure_loaded(id); // 본문 지연 로드(09-03).
        }
        let Some(item) = self.history.get(index) else {
            return;
        };
        let (item_id, item_reps, label) = (item.id, item.reps.clone(), item.label.clone());
        let by_mode = Self::reps_for_mode(&item.reps, as_);
        // ★ 원격 파일 약속(09-12 · DR-30) — 붙여넣을 때 받는다. 받는 중이면 팝업만 닫고 완료를 기다린다.
        let reps: Vec<RawRep> = match self.resolve_remote_files(item_id, &item_reps, as_) {
            Some(Resolved::Ready(r) | Resolved::Text(r)) => r,
            Some(Resolved::Fetching) => {
                self.close_popup();
                return;
            }
            None => by_mode,
        };
        if reps.is_empty() {
            // 그 모드가 무의미한 항목 — 있는 척하지 않는다(DR-31).
            eprintln!("이 항목에는 그 방식의 표현이 없습니다 — 원본(Enter)으로 붙여넣으세요");
            return;
        }
        self.close_popup();
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!("재적재: \"{label}\" — 표현 {n}개 게시");
                // ★ 부분 게시(평문만 · Linux 1단 한 표현)의 에코 = 원본 승격.
                self.history.expect_echo(index);
            }
            Err(e) => {
                eprintln!("재적재 실패: {e}");
                return;
            }
        }
        if self.paste_auto {
            // 지금 주입하지 않는다 — 팝업 파괴가 컴포지터에 닿은 뒤(다음 바퀴)에.
            let _ = self.proxy.send_event(ShellEvent::PasteAfterClose(as_));
        }
    }

    /// ★ 순차 붙여넣기(09-03 ③ — Ditto 화법): 스택 순서대로 게시→주입을 반복한다.
    ///
    /// 09-08 사용자: `as_` = 원본/평문(평문 표현이 없는 항목 — 이미지·개체 — 은 원본으로 · 건너뛰지 않는다) ·
    /// `newline` = 항목 **사이**에 Enter 한 번([`PasteInjector::send_newline`] — 마지막 뒤엔 없다).
    /// 게시/주입 사이 짧은 간격은 대상 앱이 각 붙여넣기를 처리할 시간(실측 보수치).
    /// 각 게시는 같은 내용 재복사와 같아 이력에선 **승격**으로 흡수된다(항목 증식 없음).
    fn paste_stack(&mut self, ids: &[u64], as_: PasteAs, newline: bool) {
        if !self.paste_auto {
            eprintln!("순차 붙여넣기는 자동 붙여넣기(paste.auto)가 켜져 있어야 합니다");
            return;
        }
        let n = ids.len();
        let mut done = 0usize;
        for (k, id) in ids.iter().enumerate() {
            let _ = self.ensure_loaded(*id); // 본문 지연 로드(09-03).
            let Some(item) = (0..self.history.len())
                .filter_map(|i| self.history.get(i))
                .find(|it| it.id == *id)
            else {
                continue;
            };
            let mut reps = match nclip_core::RemoteFiles::of_reps(&item.reps) {
                // ★ 원격 파일 약속(09-12) — 순차 붙여넣기는 기다리지 않는다: 캐시면 파일, 아니면 경로 텍스트.
                Some(m) => {
                    match crate::xfer::with(|x| x.all_cached(&m)).flatten() {
                        Some(paths) => {
                            let local: Vec<String> = paths
                                .iter()
                                .map(|p| p.to_string_lossy().into_owned())
                                .collect();
                            let mut r = nclip_plat::clipboard::file_reps(&local);
                            r.extend(nclip_plat::clipboard::plain_text_reps(&local.join("\r\n")));
                            r
                        }
                        None => {
                            eprintln!("순차 붙여넣기: 항목 {id}는 아직 받지 않은 원격 파일 — 경로 텍스트로");
                            nclip_plat::clipboard::plain_text_reps(&m.paths().join("\r\n"))
                        }
                    }
                }
                None => Self::reps_for_mode(&item.reps, as_),
            };
            if reps.is_empty() {
                reps = item.reps.clone(); // 평문이 없는 항목은 원본으로.
            }
            if let Err(e) = nclip_plat::clipboard::set_reps(&reps) {
                eprintln!("스택 게시 실패({id}): {e}");
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
            if let Err(e) = self.paste.restore_and_paste(as_) {
                eprintln!("스택 주입 실패({id}): {e:?}");
                continue;
            }
            done += 1;
            if k + 1 < n {
                if newline {
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    if let Err(e) = self.paste.send_newline() {
                        eprintln!("스택 줄바꿈 실패({id}): {e:?}");
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(160));
            }
        }
        println!(
            "순차 붙여넣기: {done}/{n}개 · {} · 줄바꿈 {}",
            match as_ {
                PasteAs::Plain => "평문",
                _ => "원본",
            },
            if newline { "있음" } else { "없음" }
        );
    }

    /// ★ 원격 파일 항목(DR-30 약속)의 붙여넣기 해석 — 항목에 약속이 없으면 `None`(보통 경로).
    ///
    /// | 결과 | 뜻 |
    /// |---|---|
    /// | `Ready(reps)` | 캐시에 전부 있다 — 실파일 표현(+경로 텍스트) |
    /// | `Fetching` | 지금 받기 시작했다 — 끝나면 [`on_xfer_done`](Self::on_xfer_done)이 게시·주입 |
    /// | `Text(reps)` | 파일로는 못 준다(상한·끔·오프라인·경로만 모드) — 경로 텍스트 + 로그(docs/26 §4-2) |
    fn resolve_remote_files(
        &mut self,
        item_id: u64,
        reps: &[RawRep],
        as_: PasteAs,
    ) -> Option<Resolved> {
        let m = nclip_core::RemoteFiles::of_reps(reps)?;
        let text_of =
            |paths: &[String]| nclip_plat::clipboard::plain_text_reps(&paths.join("\r\n"));
        if as_.is_text_only() {
            // ★ 경로만·평문 — 원격 내용을 끌어오지 않는다(회선 절약 · docs/26 §4-5).
            //   ⚠️ 09-13 수정: 평문이 여기서 안 갈라져 `원본`과 같은 가지로 흘렀다 —
            //   파일 표현이 만들어지고 **Linux는 표현 하나만 게시**(`pick_rep`)라
            //   그 파일 표현이 이겨 텍스트가 통째로 사라졌다(사용자 실기 09-13).
            return Some(Resolved::Text(text_of(&m.paths())));
        }
        let r = crate::xfer::with(|x| x.fetch(item_id, &m, crate::xfer::Prio::Fg))
            .unwrap_or(crate::xfer::Fetch::Disabled);
        use crate::xfer::Fetch as F;
        Some(match r {
            F::Ready(paths) => {
                let local: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                let mut out = nclip_plat::clipboard::file_reps(&local);
                if as_ != PasteAs::Object {
                    out.extend(text_of(&local));
                }
                println!(
                    "파일 공유: \"{}\" 캐시 적중 — 파일 {}개 게시",
                    m.origin_name,
                    local.len()
                );
                Resolved::Ready(out)
            }
            F::Started(n) => {
                let _ = crate::xfer::with(|x| x.set_pending_paste(item_id, as_));
                println!(
                    "파일 공유: {}에서 파일 {}개({}MB) 받는 중 — 완료되면 클립보드에 올립니다",
                    m.origin_name,
                    n.max(1),
                    m.total_bytes() / 1_048_576
                );
                let lang = current_lang();
                self.tray.notify(
                    tr(lang, Msg::XferPanel),
                    tr(lang, Msg::NotifyFilesFetching),
                    true,
                    "",
                );
                self.on_xfer_tick();
                Resolved::Fetching
            }
            F::TooLarge { total, max } => {
                eprintln!(
                    "파일 공유: 파일 내용 대신 경로만 — 합계 {}MB가 상한 {}MB를 넘습니다 · 조치: 설정 → 동기화 → \"붙여넣기 시 최대 크기\"를 올리거나 원본 기기에서 직접 복사",
                    total / 1_048_576,
                    max / 1_048_576
                );
                Resolved::Text(text_of(&m.paths()))
            }
            F::Disabled => {
                eprintln!("파일 공유: 경로만 — 파일 내용 받기가 꺼져 있습니다(설정 → 동기화)");
                Resolved::Text(text_of(&m.paths()))
            }
            F::Offline => {
                eprintln!(
                    "파일 공유: 파일 받기 불가 — 원본 기기 {} 오프라인 · 조치: 그 기기를 켜거나 자동 캐시 상한을 올리세요(경로 글자로 올립니다)",
                    m.origin_name
                );
                Resolved::Text(text_of(&m.paths()))
            }
        })
    }

    /// ★ 전송 진행(09-12 · 펌프 250ms) — 패널·배지·트레이 갱신(UI 스레드는 스냅숏만 복사한다).
    fn on_xfer_tick(&mut self) {
        self.main.set_xfers(crate::xfer::views());
        self.refresh_tray();
    }

    /// ★ 항목 전송 종결 — 전부 완료면 캐시에서 게시(+짧으면 주입) · 실패면 알림.
    /// 캐시된 파일 항목을 **OS 클립보드에 게시**(파일 표현 + 경로 텍스트) — 성공이면 `true`.
    /// 되읽힌 캡처는 `on_captured`의 캐시 에코 승격이 흡수하고, `from_reps`는 캐시 경로 페이로드를 만들지 않는다
    /// (09-12 4·5차) — 그래서 여기서 게시해도 2PC 연쇄가 열리지 않는다.
    fn publish_cached(&mut self, item_id: u64, as_: PasteAs) -> bool {
        let _ = self.ensure_loaded(item_id);
        let Some(m) = self
            .history
            .get_by_id(item_id)
            .and_then(|it| nclip_core::RemoteFiles::of_reps(&it.reps))
        else {
            return false;
        };
        let Some(paths) = crate::xfer::with(|x| x.all_cached(&m)).flatten() else {
            return false;
        };
        let local: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let reps = cached_reps(&local, as_);
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!(
                    "파일 공유: 받기 완료 — 파일 {}개 게시(표현 {n}개)",
                    local.len()
                );
                if let Some(pos) = (0..self.history.len())
                    .find(|&i| self.history.get(i).is_some_and(|it| it.id == item_id))
                {
                    self.history.expect_echo(pos);
                }
                true
            }
            Err(e) => {
                eprintln!("파일 공유: 클립보드 게시 실패({e}) — 캐시에는 있습니다");
                false
            }
        }
    }

    fn on_xfer_done(&mut self, item_id: u64) {
        let lang = current_lang();
        let done = crate::xfer::with(|x| x.item_done(item_id)).unwrap_or(false);
        let pending = crate::xfer::with(|x| x.take_pending_paste(item_id)).flatten();
        if done && pending.is_none() {
            // ★ 사전 캐시 완료(09-13 사용자 — "다른 PC에서 복사한 파일이 바로 붙여넣기되지 않는다"):
            //   그 항목이 **아직 이력 맨 앞**(그 뒤 아무것도 복사하지 않음)이면 클립보드에 게시한다 — DR-28
            //   "수신측은 바로 시스템 클립보드에 반영"을 파일에도. 맨 앞이 아니면 사용자의 더 새 클립보드를
            //   덮지 않는다(배지 `[캐시됨]` · 고르면 즉시). 09-12 4차가 껐던 게시를 5차 구조 가드 위에서 되살린 것.
            let newest = self.history.get(0).is_some_and(|it| it.id == item_id);
            if newest {
                if self.publish_cached(item_id, PasteAs::Original) {
                    println!(
                        "파일 공유: 사전 캐시 완료 — 최신 항목이라 클립보드 게시(이 PC에서 바로 붙여넣기 가능)"
                    );
                }
            } else {
                println!(
                    "파일 공유: 사전 캐시 완료 — 항목 {item_id}(최신 항목이 아니라 게시 없음 · 고르면 즉시)"
                );
            }
        } else if done {
            let as_ = pending.map_or(PasteAs::Original, |(a, _)| a);
            if self.publish_cached(item_id, as_) {
                // ★ 붙여넣기가 기다렸고 금방 끝났으면 주입 — 늦었으면 알림만(사용자는 이미 딴 데 있다).
                let injected = matches!(pending, Some((_, dt)) if dt <= crate::xfer::PASTE_INJECT_WINDOW)
                    && self.paste_auto
                    && self.paste.restore_and_paste(as_).is_ok();
                if !injected {
                    self.tray.notify(
                        tr(lang, Msg::XferPanel),
                        &tr(lang, Msg::NotifyFilesReady).replacen("{}", paste_key_label(), 1),
                        false,
                        "",
                    );
                }
            }
        } else if let Some(why) = crate::xfer::with(|x| x.item_failure(item_id)).flatten() {
            eprintln!(
                "파일 공유: 받기 실패 — {why} · 조치: 패널에서 재시도하거나 원본 기기를 확인하세요"
            );
            // 알림은 붙여넣기를 기다리던 경우만(사전 캐시 실패는 로그·배지로 충분).
            if why != "stopped" && pending.is_some() {
                self.tray
                    .notify(tr(lang, Msg::NotifyFilesFailed), &why, false, "");
            }
        }
        self.on_xfer_tick();
        self.main.on_history_changed(&self.history);
        if self.popup.is_open() {
            self.popup.on_history_changed(&self.history);
        }
    }

    /// ★ 시작·복원 뒤 한 번(09-12) — 내 파일 항목의 경로를 제안 목록에 · 원격 약속 항목은 사전 캐시 후보로.
    fn seed_xfer(&mut self) {
        let mut offered: Vec<String> = Vec::new();
        let mut promises: Vec<(u64, nclip_core::RemoteFiles)> = Vec::new();
        let mut i = 0usize;
        while let Some(it) = self.history.get(i) {
            i += 1;
            if it.kind != nclip_core::ClipKind::Files {
                continue;
            }
            match nclip_core::RemoteFiles::of_reps(&it.reps) {
                Some(m) => promises.push((it.id, m)),
                None => offered.extend(nclip_core::paths_of(&it.reps)),
            }
        }
        crate::xfer::offer(&offered);
        let mut queued = 0usize;
        for (id, m) in &promises {
            if matches!(
                crate::xfer::with(|x| x.fetch(*id, m, crate::xfer::Prio::Bg)),
                Some(crate::xfer::Fetch::Started(n)) if n > 0
            ) {
                queued += 1;
            }
        }
        if !offered.is_empty() || !promises.is_empty() {
            println!(
                "파일 공유: 제안 경로 {}개 · 원격 약속 {}개(사전 캐시 대기 {queued})",
                offered.len(),
                promises.len()
            );
        }
    }

    /// 팝업이 닫힌 다음 바퀴 — 포커스 복원 + 키 주입.
    fn paste_now(&mut self, as_: PasteAs) {
        match self.paste.restore_and_paste(as_) {
            Ok(()) => println!("붙여넣기: 포커스 복원 + 키 주입 ok"),
            Err(e) => eprintln!("붙여넣기 실패: {e:?} — 클립보드에는 실려 있습니다(Ctrl+V)"),
        }
    }

    /// ★ 이미지로 복사(09-03) — 리치 런(색·굵기)을 흰 바탕 비트맵으로 렌더해
    /// PNG+`CF_DIB`로 게시한다. PPT·Word에 Ctrl+V 하면 그림으로 붙는다.
    fn copy_as_image(&mut self, id: u64) {
        let _ = self.ensure_loaded(id);
        let Some(item) = (0..self.history.len())
            .filter_map(|i| self.history.get(i))
            .find(|it| it.id == id)
        else {
            return;
        };
        let lines = nclip_core::richtext::html_runs_of(&item.reps, 500).unwrap_or_else(|| {
            let text = crate::main_win::plain_of(&item.reps)
                .or_else(|| nclip_core::capture::svg_text(&item.reps))
                .unwrap_or_else(|| item.label.clone());
            crate::render_img::plain_runs(&text)
        });
        let imgs = crate::main_win::decode_inline_images(&lines);
        let Some((w, h, rgba)) = crate::render_img::render_runs(&self.font, &lines, &imgs) else {
            eprintln!("이미지 렌더 실패 — 내용이 비었거나 너무 큽니다");
            return;
        };
        let mut reps = vec![nclip_core::RawRep {
            format: "CF_DIB".to_string(),
            data: crate::render_img::dib_from_rgba(w, h, &rgba),
        }];
        if let Some(png) = nclip_plat::imgdec::encode_raw_isolated(w, h, &rgba) {
            reps.insert(
                0,
                nclip_core::RawRep {
                    format: "PNG".to_string(),
                    data: png,
                },
            );
        }
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => println!("이미지로 복사: {w}×{h} — 표현 {n}개 게시"),
            Err(e) => eprintln!("이미지로 복사 실패: {e}"),
        }
        self.release_body(id);
    }

    /// 팝업을 닫으며 마지막 크기를 저장한다(09-02 — 다음 열기가 이어받는다).
    fn close_popup(&mut self) {
        if let Some((w, h)) = self.popup.last_size() {
            let now = Instant::now();
            self.app.conf.set("ui.popup_w", w.to_string(), now);
            self.app.conf.set("ui.popup_h", h.to_string(), now);
        }
        self.popup.close();
    }

    /// 팝업 토글(전역 단축키) — 열 때 **먼저** 대상 포커스를 기억한다.
    fn toggle_popup(&mut self, el: &ActiveEventLoop) {
        if self.popup.is_open() {
            println!("팝업: 닫기(토글)");
            self.close_popup();
            return;
        }
        println!("팝업: 열기 — 이력 {}개", self.history.len());
        self.popup
            .set_view_code(self.app.conf.state.get("ui.popup_view"));
        self.popup
            .set_dedup(self.app.conf.state.get("ui.dedup_view") != "off");
        // ★ 마지막 크기 복원(09-02) — 값이 없으면 기본 크기.
        let pw: u32 = self.app.conf.state.get("ui.popup_w").parse().unwrap_or(0);
        let ph: u32 = self.app.conf.state.get("ui.popup_h").parse().unwrap_or(0);
        self.popup
            .set_pref_size((pw >= 200 && ph >= 160).then_some((pw, ph)));
        self.popup.set_theme(self.app.theme());
        // ★ 팝업이 뜨기 전의 포그라운드가 붙여넣기 대상이다(K-1 — 순서가 전부).
        if !self.paste.capture_focus() {
            eprintln!("대상 창을 기억하지 못했습니다 — 선택해도 주입은 생략됩니다");
        }
        self.popup
            .open(el, nclip_plat::tray::cursor_pos(), &self.history);
    }
}

impl ApplicationHandler<ShellEvent> for Shell {
    fn resumed(&mut self, _el: &ActiveEventLoop) {
        // ★ 시작은 트레이만 — 창은 트레이 "열기"/단축키에서 연다(상주 앱의 조용한 출발).
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.main.window_id() == Some(id) {
            // ★ 이동·크기 변경 때도 기하를 저장한다(09-04 사용자 — 강제 종료·크래시 뒤에도 같은 자리). 최소화(-32000)는 제외.
            if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                if let Some((x, y, _, _)) = self.main.geometry() {
                    if x > -20000 && y > -20000 {
                        self.save_main_geom();
                    }
                }
            }
            match self.main.handle_event(&event) {
                MainAction::None => {}
                MainAction::Close => self.close_main(),
                MainAction::QueryChanged => self.main.on_history_changed(&self.history),
                MainAction::OpenSettings => self.open_settings(el),
                MainAction::Copy { id, as_ } => self.copy_from_main(id, as_),
                MainAction::SetViewMode(code) => {
                    self.app
                        .conf
                        .set("ui.view_mode", code.to_string(), Instant::now());
                }
                MainAction::SaveEdit { id, text } => self.save_edit(id, &text),
                MainAction::ToggleAlwaysTop => {
                    let on = self.app.conf.state.get("ui.always_on_top") != "on";
                    self.app.conf.set(
                        "ui.always_on_top",
                        if on { "on" } else { "off" }.to_string(),
                        Instant::now(),
                    );
                    self.main.apply_always_top(on);
                    // ★ 설정 창도 따라간다(09-04) — 메인창 위에 있어야 한다.
                    self.app.set_level(on && self.main.window_id().is_some());
                    println!("최상위 고정: {}", if on { "켜짐" } else { "꺼짐" });
                    if on && !self.atop_effective {
                        println!(
                            "최상위 고정: Wayland 창에는 즉시 적용되지 않습니다 — 재시작하면 X11 창으로 적용됩니다"
                        );
                    }
                }
                MainAction::SearchMode(v) => self.set_find_mode(v),
                MainAction::ToggleWatch => {
                    self.watch_off = !self.watch_off;
                    self.main.apply_watch_off(self.watch_off);
                    println!(
                        "클립보드 감시: {}",
                        if self.watch_off {
                            "꺼짐 — 툴바 토글로 다시 켭니다(이 세션만)"
                        } else {
                            "켜짐"
                        }
                    );
                }
                // ★ 전송 패널 토글(09-13 사용자 — 툴바 바닥 연결/⚙ 위 버튼) — 창 안 상태(영속 없음).
                MainAction::ToggleXfer => self.main.toggle_xfer(),
                MainAction::TogglePreview => {
                    let on = self.app.conf.state.get("ui.preview_open") != "on";
                    self.app.conf.set(
                        "ui.preview_open",
                        if on { "on" } else { "off" }.to_string(),
                        Instant::now(),
                    );
                    self.main.apply_preview(on);
                }
                // ★ 중복 제외 보기(09-04) — 영속 + 행 재구성.
                MainAction::DedupView(on) => {
                    self.app.conf.set(
                        "ui.dedup_view",
                        if on { "on" } else { "off" }.to_string(),
                        Instant::now(),
                    );
                    self.main.apply_dedup(on);
                    self.main.on_history_changed(&self.history);
                    self.popup.set_dedup(on); // 팝업도 같은 규칙(09-04)
                    self.popup.on_history_changed(&self.history);
                }
                MainAction::CopyImage(id) => self.copy_as_image(id),
                // ★ 전송 패널(09-12) — 파일 단위 중지·재시도·지우기 · 전체 중지.
                MainAction::XferStop(req) => {
                    if crate::xfer::with(|x| x.stop(req)).unwrap_or(false) {
                        println!("파일 공유: 전송 {req} 중지");
                    }
                    self.on_xfer_tick();
                }
                MainAction::XferRetry(req) => {
                    if crate::xfer::with(|x| x.retry(req)).unwrap_or(false) {
                        println!("파일 공유: 전송 {req} 재시도(이어 받기)");
                    }
                    self.on_xfer_tick();
                }
                MainAction::XferClear => {
                    let _ = crate::xfer::with(|x| x.clear_finished(std::time::Duration::ZERO));
                    self.on_xfer_tick();
                }
                MainAction::XferStopAll => {
                    let n = crate::xfer::with(|x| x.stop_all()).unwrap_or(0);
                    println!("파일 공유: 전송 {n}개 중지");
                    self.on_xfer_tick();
                }
                MainAction::Delete(id) => {
                    if self.history.remove(id) {
                        let _ = crate::xfer::with(|x| x.forget_item(id));
                        self.thumbs.borrow_mut().remove(id);
                        self.search_idx.borrow_mut().remove(&id);
                        self.store.remove(id);
                        self.main.on_history_changed(&self.history);
                        self.refresh_tray();
                        self.main.on_history_changed(&self.history);
                    }
                }
                MainAction::TogglePin(id) => {
                    let now = self
                        .history
                        .get_by_id(id)
                        .map(|it| !it.pinned)
                        .unwrap_or(false);
                    if self.history.set_pinned(id, now) {
                        // ★ 고정 = 섬네일 캐시 축출 제외(DR-42 ①).
                        self.thumbs.borrow_mut().set_keep(id, now);
                        // ★ 핀 영속 = 같은 id Add 재기록(재생 시 교체 — docs/28 §4).
                        if let Some(it) = self.history.get_by_id(id) {
                            let __sid = it.id;
                            self.store_add(__sid);
                        }
                        self.main.on_history_changed(&self.history);
                    }
                }
            }
            self.pump_preview();
            return;
        }
        // 팝업 창의 이벤트는 팝업으로 — 나머지(설정 창)는 App으로.
        if self.popup.window_id() == Some(id) {
            match self.popup.handle_event(&event, &self.history) {
                PopupAction::None => {}
                PopupAction::Close => self.close_popup(),
                PopupAction::SearchMode(v) => self.set_find_mode(v),
                // ★ Ctrl+,/⌘,(09-07) — 팝업은 닫고(포커스 복원 · 붙여넣기 없음) 설정 창을 띄운다.
                PopupAction::OpenSettings => {
                    self.close_popup();
                    self.open_settings(el);
                }
                PopupAction::Pick { index, as_ } => self.pick(index, as_),
                PopupAction::PickStack { ids, as_, newline } => {
                    self.close_popup();
                    let _ = self
                        .proxy
                        .send_event(ShellEvent::PasteStack { ids, as_, newline });
                }
            }
            return;
        }
        ApplicationHandler::window_event(&mut self.app, el, id, event);
        // ★ 언어 즉시 반영(09-02) — 설정 창이 바꾼 전역을 트레이·메인·팝업에도.
        if self.app.take_ui_refresh() {
            self.refresh_tray();
            self.main.on_history_changed(&self.history);
            self.popup.redraw_public();
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: ShellEvent) {
        match ev {
            ShellEvent::Open => {
                // ★ 08-30 사용자 확정: 트레이 좌클릭/"열기" = **메인창**(설정은 메인 ⚙).
                let theme = self.app.theme();
                // ★ 저장 기하가 지금 모니터 밖이면(2→1 모니터 등) 보이는 자리로 보정(09-04 사용자).
                let geom = self.saved_main_geom().map(|g| fit_geom_to_monitors(el, g));
                let view = self.app.conf.state.get("ui.view_mode").to_string();
                let atop = self.app.conf.state.get("ui.always_on_top") == "on";
                let pv = self.app.conf.state.get("ui.preview_open") == "on";
                let dd = self.app.conf.state.get("ui.dedup_view") != "off"; // 기본 켬(09-04)
                self.main.open(
                    el,
                    &self.history,
                    theme,
                    geom,
                    crate::main_win::OpenOpts {
                        view_code: &view,
                        always_top: atop,
                        preview_open: pv,
                        dedup_view: dd,
                    },
                );
            }
            ShellEvent::OpenSettings => self.open_settings(el),
            ShellEvent::Quit => {
                self.save_main_geom();
                el.exit();
            }
            ShellEvent::Hotkey(id) => match id {
                nclip_core::hotkey::ID_PASTE_PLAIN => self.paste_plain_hotkey(),
                _ => self.toggle_popup(el),
            },
            ShellEvent::RestoreClipboard(id) => self.restore_clipboard(id),
            ShellEvent::PasteAfterClose(as_) => self.paste_now(as_),
            ShellEvent::PasteStack { ids, as_, newline } => self.paste_stack(&ids, as_, newline),
            // ★ 연결 표시(09-04) — 릴레이 이벤트든 LAN 세션 변화(SyncTick)든 **같은 판정**으로 갱신:
            //   동기화 Off = 숨김 · 켜짐 = 릴레이 연결 ∨ LAN 피어 연결(릴레이 None·프로필 실행도 표시된다).
            ShellEvent::ThumbReady { id, w, h, rgba } => {
                let pinned = self.history.get_by_id(id).is_some_and(|it| it.pinned);
                self.thumbs.borrow_mut().insert(
                    id,
                    nclip_ctl::theme::IconImage::from_rgba(w, h, rgba),
                    pinned,
                );
                self.main.redraw_now();
                self.popup.redraw_now();
            }
            ShellEvent::ThumbFailed(id) => self.thumbs.borrow_mut().fail(id),
            ShellEvent::SearchText { id, text } => {
                if self.history.get_by_id(id).is_some() {
                    self.search_idx.borrow_mut().insert(id, Rc::from(text));
                }
            }
            ShellEvent::ThumbEncoded { id, w, h, png } => self.on_thumb_encoded(id, w, h, png),
            ShellEvent::ThumbMigrated => {
                // ★ 인덱스를 바로 줄인다(30 §5) — 구본 RGBA 레코드가 죽은 이벤트로 남아 있다.
                let live: Vec<StoredItem> = (0..self.history.len())
                    .filter_map(|i| self.history.get(i))
                    .map(to_stored)
                    .collect();
                self.store.compact_now(&live);
                println!(
                    "섬네일 마이그레이션 완료: {}개 → PNG blob · 인덱스 압축",
                    self.thumb_migrated
                );
            }
            ShellEvent::SyncTick => self.refresh_sync_indicator(),
            ShellEvent::SyncState(on) => {
                let _ = on; // 값은 판정에 안 쓴다(러너 상태·LAN 피어를 직접 본다) — 이벤트는 깨우기용.
                self.refresh_sync_indicator();
            }
            ShellEvent::SystemTheme => {
                self.app.apply_theme();
                self.popup.set_theme(self.app.theme());
                self.main.set_theme(self.app.theme());
                println!(
                    "테마: OS 선호 변경 → {}(ui.theme = {})",
                    if self.app.theme().is_dark {
                        "dark"
                    } else {
                        "light"
                    },
                    self.app.conf.state.get("ui.theme")
                );
            }
            ShellEvent::HotkeyStatus(ok) => {
                if ok {
                    println!("전역 단축키: {}", nclip_plat::tray::hotkey_label());
                } else {
                    eprintln!(
                        "⚠️ 전역 단축키({}) 등록 일부 실패 — {}. 설정 → 단축키에서 조합을 바꾸세요",
                        nclip_plat::tray::hotkey_label(),
                        nclip_plat::tray::hotkey_failure_hint()
                    );
                }
            }
            ShellEvent::Captured(snap) => self.on_captured(snap, None),
            ShellEvent::SyncItem {
                from,
                reps,
                summary,
                skip_hash,
            } => self.apply_remote(&from, reps, &summary, skip_hash),
            ShellEvent::Recent(i) => self.repost(i),
            ShellEvent::XferTick => self.on_xfer_tick(),
            ShellEvent::XferDone { item_id } => self.on_xfer_done(item_id),
        }
        self.pump_preview();
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        ApplicationHandler::about_to_wait(&mut self.app, el);
        // ★ 동기화 재기동(09-03) — 설정 창 Test 성공이 러너 (재)접속을 요청한다.
        if self.app.take_sync_respawn() {
            crate::sync_cmd::spawn_if_enabled(&self.app.conf, self.proxy.clone());
        }
        // ★ 단축키 설정 동기(09-04) — 바뀌면 플랫폼 재등록(Windows 즉시) · 창 안 키맵(09-08)은 창에 새 표.
        self.apply_hotkeys(false);
        self.apply_keymap();
        // ★ 검색 방식 동기(09-04 · `find.mode`) — 설정 창에서 바꾸면 다음 박동에 열린 창이 다시 거른다.
        let mode = nclip_core::search::Mode::from_code(self.app.conf.state.get("find.mode"));
        if self.main.set_search_mode(mode) {
            self.main.on_history_changed(&self.history);
        }
        if self.popup.set_search_mode(mode) {
            self.popup.on_history_changed(&self.history);
        }
        // ★ 뷰포트 섬네일 디코드 요청 처리(09-04 · 30 §4).
        self.pump_thumbs();
        // ★ 메모리 GC(09-04 · DR-42) — 30초 주기 · 상한 초과 즉시.
        self.gc_memory(false);
        // ★ 기록 모두 삭제(09-04 사용자 — 설정 고급 · 2단계 확인 통과) — 고정 제외 전부 · 저장소까지.
        if self.app.take_clear_history() {
            let gone = self.history.remove_unpinned();
            for id in &gone {
                self.store.remove(*id);
                self.search_idx.borrow_mut().remove(id);
                self.thumbs.borrow_mut().remove(*id);
            }
            println!("기록 모두 삭제: {}개 (고정 항목 유지)", gone.len());
            self.main.on_history_changed(&self.history);
            self.popup.on_history_changed(&self.history);
            self.refresh_tray();
        }
        // ★ 캐럿 깜박임(09-02) — 검색창 띄운 창이 있으면 500ms 위상. 설정 창 페이드와
        //   겹칠 땐 더 짧은 쪽 데드라인이 이기지만, 우리가 덮어도 250ms 주기라 체감 무해.
        let searching = self.main.window_id().is_some() || self.popup.window_id().is_some();
        if searching {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let phase = (now_ms / 500) % 2 == 0;
            self.main.set_caret_phase(phase);
            self.popup.set_caret_phase(phase);
            // ★ 스크롤바 자동 숨김 페이드(09-02) — 같은 박동에 얹는다. ★ 툴바 hover 페이드 중이면 16ms(09-04).
            let animating = self.main.tick_ui(now_ms) | self.popup.tick_ui(now_ms);
            let rem = if animating { 16 } else { 500 - (now_ms % 500) };
            el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(rem.max(16)),
            ));
        }
    }
}

/// 트레이 + 감시 + 설정 창 상주. 종료는 트레이 메뉴에서.
/// ★ 최상위 고정 × Wayland(09-02) — Wayland 프로토콜에는 "항상 위" 요청 자체가 없다
///   (winit wayland `set_window_level` = no-op). `ui.always_on_top=on`이면 **창 백엔드만**
///   XWayland(X11)로 강제해 `_NET_WM_STATE_ABOVE`로 동작시킨다. 환경변수는 건드리지
///   않으므로 주입(포털 RemoteDesktop)·감시(도구 판별)·트레이(D-Bus) 판정은 불변.
///   XWayland(`DISPLAY`)가 없으면 강제하지 않는다(정직 강등).
#[cfg(all(unix, not(target_os = "macos")))]
fn want_x11_windows(conf: &Settings) -> bool {
    conf.state.get("ui.always_on_top") == "on"
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_some()
}

/// X11 백엔드 선결 조건 — winit x11은 `libxkbcommon-x11`을 dlopen하며 **없으면 패닉**한다
///   (09-02 실기 — 미설치 PC에서 기동 자체가 죽었다). 시작 전에 존재를 확인해 정직 강등.
#[cfg(all(unix, not(target_os = "macos")))]
fn x11_backend_ready() -> bool {
    [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib",
        "/usr/local/lib",
    ]
    .iter()
    .any(|d| {
        std::path::Path::new(d)
            .join("libxkbcommon-x11.so.0")
            .exists()
    })
}

/// 설정 `key.*` → (등록 목록, 안내 라벨, 변경 감지 서명) — 부팅(트레이 기동 전)과
/// 런타임 적용(`Shell::apply_hotkeys`)이 같은 계산을 쓴다(09-04).
fn hotkey_conf(conf: &Settings) -> (Vec<(u32, nclip_core::hotkey::Hotkey)>, String, String) {
    use nclip_core::hotkey::{Hotkey, ACTIONS};
    let mut list = Vec::new();
    let mut labels = Vec::new();
    let mut sig = String::new();
    for (key, id, _) in ACTIONS {
        let v = conf.state.get(key).trim().to_string();
        sig.push_str(&v);
        sig.push('|');
        if let Some(h) = Hotkey::parse(&v) {
            labels.push(format!(
                "{}={}",
                match *id {
                    nclip_core::hotkey::ID_PASTE_PLAIN => "평문",
                    nclip_core::hotkey::ID_OPEN_ALT => "팝업(보조)",
                    _ => "팝업",
                },
                h.display(cfg!(target_os = "macos"))
            ));
            list.push((*id, h));
        }
    }
    (list, labels.join(" · "), sig)
}

pub(crate) fn run() {
    // ★ 설정을 먼저 — UI 글꼴(`ui.font_family`)이 폰트 선택을 좌우한다(09-01 "JetBrains Mono").
    let mut conf = Settings::load();
    crate::conf::apply_lang(&conf);
    let Some(font) = crate::conf::load_ui_font(&conf) else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다.");
        std::process::exit(1);
    };

    sync_autostart(&mut conf);

    // ★ T-12e4 단일 인스턴스(09-03) — 이미 상주 중이면 "열기"만 위임하고 조용히 끝낸다
    //   (자동 시작 상주 + 런처 재실행 = 감시 2중·트레이 2개이던 관찰의 처방).
    let single_guard = nclip_plat::single::acquire(
        &crate::conf::data_dir().join("instance.lock"),
        crate::conf::profile(),
    );
    if single_guard.is_none() {
        nclip_plat::single::signal_open(crate::conf::profile());
        println!("이미 실행 중 — 기존 인스턴스에 열기를 위임했습니다");
        return;
    }

    // ★ 최상위 고정이 지금 백엔드에서 실제로 먹는가 — Wayland 네이티브 창이면 false.
    #[cfg(all(unix, not(target_os = "macos")))]
    let x11_windows = want_x11_windows(&conf) && x11_backend_ready();
    #[cfg(all(unix, not(target_os = "macos")))]
    let atop_effective = std::env::var_os("WAYLAND_DISPLAY").is_none() || x11_windows;
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    let atop_effective = true;

    let mut el_builder = EventLoop::<ShellEvent>::with_user_event();
    // ★ Dock 아이콘(T-12e mac · 09-03) — 끔 = 처음부터 Accessory로 기동(Dock 깜빡임 없음).
    //   실행 중 토글은 설정 창이 `dock::set_dock_visible`로 즉시 반영한다.
    #[cfg(target_os = "macos")]
    if conf.state.get("ui.dock_icon") == "off" {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS as _};
        el_builder.with_activation_policy(ActivationPolicy::Accessory);
        println!("Dock 아이콘: 숨김(ui.dock_icon = off) — 메뉴 막대에서만 엽니다");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if x11_windows {
        use winit::platform::x11::EventLoopBuilderExtX11 as _;
        el_builder.with_x11();
        println!("최상위 고정: Wayland 세션 — 창을 XWayland(X11) 백엔드로 띄워 적용합니다");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if !atop_effective && conf.state.get("ui.always_on_top") == "on" {
        if want_x11_windows(&conf) && !x11_backend_ready() {
            eprintln!(
                "⚠️ 최상위 고정: libxkbcommon-x11 미설치 — 창을 Wayland로 띄웁니다. \
`sudo apt install libxkbcommon-x11-0` 후 재시작하면 적용됩니다"
            );
        } else {
            eprintln!("⚠️ 최상위 고정: XWayland(DISPLAY) 없음 — 이 세션에서는 적용할 수 없습니다");
        }
    }
    let Ok(el) = el_builder.build() else {
        eprintln!("이벤트 루프 생성 실패");
        std::process::exit(1);
    };
    el.set_control_flow(ControlFlow::Wait);

    // 이력 상한·메뉴 개수 — 복원이 쓴다(아래).
    let cap: usize = conf.state.get("store.max_items").parse().unwrap_or(1000);
    let tray_n: usize = conf.state.get("ui.tray_recent_n").parse().unwrap_or(8);

    // ★ 영속(T-16) — 설정 옆 store/ 에서 이력을 복원한다. 열기 실패는
    //   NullStore 강등(이력은 세션 한정 — 안 뜨는 것보다 낫다 · DR-31).
    let (mut store, history): (Box<dyn HistoryStore>, History) =
        match FileStore::open(&crate::conf::data_dir().join("store")) {
            Ok(rep) => {
                if rep.archived {
                    eprintln!(
                        "⚠️ 저장소: 기기 키 불일치 — 기존 기록을 .locked로 보관하고 새로 시작합니다"
                    );
                }
                let mut fs = rep.store;
                let items: Vec<HistoryItem> = fs.load().into_iter().map(to_history).collect();
                println!("저장소: {}개 복원 (암호화 기본 · DR-38)", items.len());
                (Box::new(fs), History::from_items(cap, items))
            }
            Err(e) => {
                eprintln!("⚠️ 저장소를 열 수 없음: {e} — 이번 세션은 메모리로만 보관합니다");
                (Box::new(NullStore), History::new(cap))
            }
        };
    // 복원 직후 상한 축출분을 저장소에도 반영한다(설정을 줄여 놓았던 경우).
    let mut history = history;
    for id in history.drain_evicted() {
        store.remove(id);
    }

    // ★ 단축키 목록을 트레이 기동 **전에** 넘긴다(09-04 mac 실기 "⇧⌥C 무동작") —
    //   mac(Carbon)·Linux(포털)는 기동 때 한 번만 읽는다: 뒤에 넘기면 빈 목록으로 등록돼
    //   단축키가 전부 죽는다. Windows만 재등록 메시지가 있어 이 순서 결함이 가려져 있었다.
    let (hk_list, hk_label, hk_sig) = hotkey_conf(&conf);
    nclip_plat::tray::set_hotkeys(hk_list, hk_label);

    // 트레이 — 이벤트는 프록시로 메인 루프에 되돌린다(beep 호스트 문법).
    //   ★ 복원을 먼저 끝내 첫 우클릭부터 최근이 보인다(09-01 D1 — 예전엔 빈 메뉴로 떴다).
    let proxy = el.create_proxy();
    let Some(tray) = spawn(
        content(history.len(), history.recent_labels(tray_n), false),
        move |ev| {
            let _ = proxy.send_event(match ev {
                TrayEvent::Quit => ShellEvent::Quit,
                TrayEvent::Recent(i) => ShellEvent::Recent(i),
                TrayEvent::Hotkey(id) => ShellEvent::Hotkey(id),
                TrayEvent::HotkeyStatus(ok) => ShellEvent::HotkeyStatus(ok),
                TrayEvent::Open | TrayEvent::OpenTarget(_) => ShellEvent::Open,
                TrayEvent::Settings => ShellEvent::OpenSettings,
            });
        },
    ) else {
        eprintln!(
            "트레이를 띄울 수 없습니다 — {}",
            nclip_plat::tray::tray_failure_hint()
        );
        std::process::exit(1);
    };

    // ★ Wayland 키 주입 권한(포털 RemoteDesktop) — 시작 때 한 번 받아 둔다(토큰 영구).
    //   대화창 응답까지 막히므로 별도 스레드 — 트레이 기동을 기다리게 하지 않는다.
    {
        // ★ 09-05: 데이터 폴더가 아니라 **고정 자리**(사용자 설정 폴더) — 개발 빌드 ↔ 설치본을
        //   오갈 때 승인이 되풀이되던 것을 없앤다(권한은 사용자·앱 단위지 데이터 사본 단위가 아니다).
        //   ★ Linux 전용 인자다 — Windows·mac은 `warm_up`이 이 값을 쓰지 않으므로 **경로를 만들지도 않는다**
        //   (포터블 설치가 쓰지도 않을 `%APPDATA%`·`Application Support`를 만들어선 안 된다).
        #[cfg(target_os = "linux")]
        let token = Some(crate::conf::portal_token_path());
        #[cfg(not(target_os = "linux"))]
        let token: Option<std::path::PathBuf> = None;
        let _ = std::thread::Builder::new()
            .name("nclip-paste-warmup".into())
            .spawn(move || match nclip_plat::paste::warm_up(token) {
                Ok(()) => println!("키 주입 권한: ok"),
                Err(e) => eprintln!(
                    "⚠️ 키 주입 권한 없음 — {e}. 선택하면 클립보드 적재까지만(Ctrl+V는 직접)"
                ),
            });
    }
    // ★ 런처 .desktop + 아이콘(Linux) — Dock이 app_id `nexa-clip`과 맞춰 우리 아이콘을 쓴다
    //   (08-30 사용자 실기 "톱니바퀴"). 멱등 · 다른 OS no-op.
    if let Err(e) = nclip_plat::autostart::install_launcher(include_bytes!(
        "../../../packaging/branding/nexa-clip-256.png"
    )) {
        eprintln!("런처 항목 설치 실패: {e}");
    }

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
        if let WatchCapability::Unsupported { reason } = watch.capability() {
            println!("클립보드 감시: 사용 불가({reason:?}) — 트레이만 동작합니다");
        }
    }

    // ★ OS 테마 변경 감시(Linux 포털 · 다른 OS는 창 이벤트) — `ui.theme = system` 추종.
    {
        let proxy = el.create_proxy();
        nclip_plat::theme::watch(move |_| {
            let _ = proxy.send_event(ShellEvent::SystemTheme);
        });
    }

    // ★ Ctrl+C = 정상 종료(트레이 메뉴 "종료"와 같은 경로) — 안 걸면 프로세스가
    //   STATUS_CONTROL_C_EXIT로 죽어 cargo가 오류처럼 찍는다(08-28 실기 오인).
    {
        let proxy = el.create_proxy();
        nclip_plat::console::on_console_quit(move || {
            let _ = proxy.send_event(ShellEvent::Quit);
        });
    }

    println!(
        "트레이 상주: ok — 좌클릭/열기 = 메인창(항목 관리) · 우클릭 = 최근 메뉴 · 설정은 메인창 ⚙"
    );
    println!(
        "창 닫기: {}",
        if conf.state.get("ui.close_to_tray") == "on" {
            "트레이로 숨김 (ui.close_to_tray = on)"
        } else {
            "앱 종료 (설정 '창을 닫아도 트레이에 남기'를 켜면 숨김)"
        }
    );
    println!("종료: 트레이 메뉴 \"종료\" 또는 Ctrl+C — 둘 다 정상 종료(설정 저장 포함)");

    let paste_auto = conf.state.get("paste.auto") == "on";
    let gate = Gate::from_state(&conf);

    // 팝업은 자기 폰트를 따로 든다(mmap 정적 데이터라 값싸다 — App이 font를 소유해서).
    let popup_font = font.clone();

    // 메인창 폰트 — 팝업과 같은 이유로 자기 것을 따로 든다(mmap 정적 데이터).
    let main_font = font.clone();
    // ★ 고정폭 글꼴(09-04) — 터미널/코드 리치 런의 Mono 슬롯(없으면 주 글꼴).
    let mono_font = crate::conf::load_mono_font(&conf, &font);
    // ★ 파일 내용 공유(09-12 · DR-30) — 캐시 관리자 + 펌프(세션이 없어도 상주 · 요청은 세션 있을 때만).
    // ★ 저장 폴더(09-13) = `sync.file_dir`(비면 OS 다운로드 폴더 아래 `Nexa Clip`) · 종전 위치는 캐시 판정용으로 기억.
    {
        let data = crate::conf::data_dir();
        let dir = crate::xfer::resolve_file_dir(conf.state.get("sync.file_dir"), &data);
        println!("파일 공유: 저장 폴더 {}", dir.display());
        crate::xfer::init(dir, Some(data.join("cache").join("files")));
    }
    crate::xfer::set_policy(xfer_policy(&conf));
    crate::xfer::spawn_pump(el.create_proxy());
    // ★ M2 동기화 기반(09-03) — 켜져 있으면 릴레이 접속 스레드 상주(상태는 proxy로 통지).
    crate::sync_cmd::spawn_if_enabled(&conf, el.create_proxy());
    // ★ 정보 화면 실행 파일 해시(09-12) — 부팅 때 워커 한 번(UI는 결과만 읽는다).
    crate::about::spawn_hash();

    {
        // ★ 둘째 실행의 "열기" 신호 → 메인창(Windows · 09-03).
        let proxy = el.create_proxy();
        nclip_plat::single::watch_open_requests(crate::conf::profile(), move || {
            println!("단일 인스턴스: 열기 위임 수신 — 메인창을 앞으로");
            let _ = proxy.send_event(ShellEvent::Open);
        });
    }

    // ★ 섬네일 캐시(09-04 · 30 §4) — 32장(384² ≈ 19MB 상한) · 메인·팝업·셸 공유.
    let thumbs: crate::thumbs::Thumbs =
        std::rc::Rc::new(std::cell::RefCell::new(crate::thumbs::ThumbCache::new(32)));
    let search_idx = crate::search_index::new_index();
    let mut shell = Shell {
        font: font.clone(),
        app: App::new(font, conf, true),
        popup: Popup::new(popup_font),
        main: MainWin::new(main_font),
        tray,
        history,
        store,
        gate,
        tray_n,
        paste: PlatformPaste::new(),
        paste_auto,
        sync_on: None,
        sync_skip: None,
        proxy: el.create_proxy(),
        atop_effective,
        last_gc: Instant::now(),
        thumbs: thumbs.clone(),
        thumb_migrated: 0,
        search_idx: search_idx.clone(),
        hotkey_sig: hk_sig,
        keymap_sig: String::new(),
        watch_off: false,
    };
    shell.apply_keymap();
    shell.main.set_mono_font(mono_font.clone());
    shell.popup.set_mono_font(mono_font);
    shell.main.set_thumbs(thumbs.clone());
    shell.popup.set_thumbs(thumbs);
    shell.main.set_search_index(search_idx.clone());
    shell.popup.set_search_index(search_idx);
    shell.start_thumb_migration();
    shell.build_search_index();
    // ★ 복원 직후 트레이 메뉴·툴팁 갱신(09-01 사용자 실기 "우클릭에 최근이 안 보임") —
    //   spawn 때는 빈 내용이었고 첫 캡처까지는 아무도 불러주지 않았다.
    shell.refresh_tray();
    shell.seed_xfer();
    if let Err(e) = el.run_app(&mut shell) {
        eprintln!("이벤트 루프 오류: {e}");
    }
    // single_guard는 여기까지 살아 있다 — 프로세스 수명만큼 인스턴스 소유 유지.
    // ★ 종료 직전 강제 수거 — "바꾸고 바로 종료하면 안 저장됨"을 막는다.
    if shell.app.conf.flush() {
        println!("설정 저장: {}", shell.app.conf.path().display());
    }
    // ★ `sec.clear_on_quit` — 켜져 있으면 종료 때 기록을 지운다(세그먼트·blob 실파일).
    if shell.app.conf.state.get("sec.clear_on_quit") == "on" {
        shell.store.wipe();
        println!("기록 비움: sec.clear_on_quit = on");
    }
    println!("종료합니다 — 이번 상주에서 {}개 보관.", shell.history.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(format: &str, data: &[u8]) -> RawRep {
        RawRep {
            format: format.into(),
            data: data.to_vec(),
        }
    }

    /// 합성된 경로 텍스트를 읽어 온다 — 평문 표현의 이름은 OS마다 다르다.
    fn text_of(reps: &[RawRep]) -> String {
        let r = reps.first().expect("표현이 있어야 한다");
        if cfg!(windows) {
            let u: Vec<u16> = r
                .data
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&u| u != 0)
                .collect();
            String::from_utf16_lossy(&u)
        } else {
            String::from_utf8_lossy(&r.data).into_owned()
        }
    }

    /// ★ 09-13 결함 ⓑ — 경로 합성이 `CF_HDROP`에만 걸려 **Windows 전용**이었다.
    /// 세 OS의 파일 표현 어느 것에서도 경로만/평문이 경로 텍스트를 내야 한다.
    #[test]
    fn path_only_synthesizes_text_on_every_os_file_rep() {
        let mut hdrop = Vec::new();
        hdrop.extend_from_slice(&20u32.to_le_bytes()); // pFiles
        hdrop.extend_from_slice(&[0u8; 12]); // pt
        hdrop.extend_from_slice(&1u32.to_le_bytes()); // fWide
        for u in "C:\\a.txt".encode_utf16().chain([0, 0]) {
            hdrop.extend_from_slice(&u.to_le_bytes());
        }
        let cases: [(&str, RawRep, &str); 3] = [
            ("windows", rep("CF_HDROP", &hdrop), "C:\\a.txt"),
            (
                "linux",
                rep("x-special/gnome-copied-files", b"copy\nfile:///tmp/a.txt"),
                "/tmp/a.txt",
            ),
            (
                "macos",
                rep("public.file-url", b"file:///Users/k/a.txt"),
                "/Users/k/a.txt",
            ),
        ];
        for (os, r, want) in cases {
            for as_ in [PasteAs::PathOnly, PasteAs::Plain] {
                let out = Shell::reps_for_mode(std::slice::from_ref(&r), as_);
                assert!(
                    !out.is_empty(),
                    "{os} {as_:?} — 빈 표현이면 아무 일도 안 난다"
                );
                assert_eq!(text_of(&out), want, "{os} {as_:?}");
            }
        }
    }

    /// 평문 표현이 이미 있으면 합성하지 않고 그것을 쓴다(종전 동작 유지).
    #[test]
    fn existing_plain_rep_wins_over_synthesis() {
        let reps = [
            rep("x-special/gnome-copied-files", b"copy\nfile:///tmp/a.txt"),
            rep("text/plain", b"eeny meeny"),
        ];
        let out = Shell::reps_for_mode(&reps, PasteAs::Plain);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].format, "text/plain");
    }

    /// ★ 09-13 — 캐시 게시도 **글자만 청한 모드에는 파일 표현을 얹지 않는다**.
    /// (얹으면 Linux 단일 게시에서 파일 표현이 이겨 텍스트가 사라진다 — 같은 결함 2호.)
    #[test]
    fn cached_reps_never_add_file_reps_for_text_modes() {
        let local = vec!["/tmp/a.txt".to_string()];
        for as_ in [PasteAs::Plain, PasteAs::PathOnly] {
            let out = cached_reps(&local, as_);
            assert!(
                out.iter().all(|r| nclip_core::is_plain_format(&r.format)),
                "{as_:?} — 파일 표현이 섞였다: {:?}",
                out.iter().map(|r| &r.format).collect::<Vec<_>>()
            );
            assert_eq!(text_of(&out), "/tmp/a.txt");
        }
        // 원본 = 파일 + 텍스트 · 개체 = 파일만(종전 동작 유지).
        assert!(cached_reps(&local, PasteAs::Original).len() > 1);
        let obj = cached_reps(&local, PasteAs::Object);
        assert!(!obj.is_empty());
        assert!(obj.iter().all(|r| !nclip_core::is_plain_format(&r.format)));
    }

    /// 파일도 SVG도 아닌 항목의 경로만 = 줄 것이 없다(정직하게 빈 표현).
    #[test]
    fn path_only_on_non_file_item_is_empty() {
        let reps = [rep("image/png", b"\x89PNG")];
        assert!(Shell::reps_for_mode(&reps, PasteAs::PathOnly).is_empty());
    }
}
