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
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::icon::{icon_rgba, ICON_SIDE};

/// 트레이 메뉴 라벨 최대 글자 수 — 길면 메뉴가 화면을 덮는다(문자 경계 절단).
const MENU_LABEL_CHARS: usize = 44;

/// 목록 썸네일 긴 변(px) — 팝업 행(30px)에 들어가는 크기의 2배(고DPI 여유).
/// ★ 09-02: 48→160 — Rich 본문 존(≈행 높이)에 그려도 흐릿하지 않게.
const THUMB_SIDE: u32 = 512; // ★ 09-02 가변 행 — 실치수 표시(최대 높이 200px)에도 선명하게.
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
    /// ★ 전역 단축키(`Ctrl+Shift+V`) — 퀵 팝업 토글.
    Hotkey,
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
        settings_label: tr(lang, Msg::TraySettings).to_string(),
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
        thumb: it.thumb.clone(),
        created_ms: it.created_ms,
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
        it.created_ms,
    )
}

/// 상주 셸 — 창·트레이·감시·이력·팝업을 winit 한 루프로.
struct Shell {
    app: App,
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
    /// 루프에 되돌려 보내는 통로(`PasteAfterClose`).
    proxy: winit::event_loop::EventLoopProxy<ShellEvent>,
}

impl Shell {
    /// 이력이 변했다 — 트레이 메뉴·툴팁을 새 내용으로.
    fn refresh_tray(&self) {
        self.tray.update(content(
            self.history.len(),
            self.history.recent_labels(self.tray_n),
        ));
    }

    /// 메인창 닫기 — `ui.close_to_tray` 정책 공유(설정 창과 동일 계약).
    /// ★ K4 미리보기 펌프 — 메인창이 원본 이미지를 원하면 지연 디코드해 넘긴다.
    fn pump_preview(&mut self) {
        if let Some(pid) = self.main.take_preview_request() {
            let img = self
                .history
                .get_by_id(pid)
                .and_then(|it| decode_image(&it.reps, PREVIEW_SIDE));
            match img {
                Some((w, h, rgba)) => self.main.set_preview_image(pid, w, h, rgba),
                // 디코드 실패(이미지 표현 없는 Object 포함) — 텍스트 폴백으로 전환.
                None => self.main.set_preview_failed(pid),
            }
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
                self.store.add(&to_stored(it));
            }
            self.main.on_history_changed(&self.history);
            self.refresh_tray();
            println!("편집 저장: 항목 {id} — 평문으로 교체");
        }
    }

    /// ★ 모드 선별 + 폴백(09-01 J2) — 파일 항목의 "경로만"은 평문 표현이 아예 없을 수
    /// 있어(탐색기 복사 = CF_HDROP뿐) 경로 목록에서 평문을 **만들어** 준다.
    fn reps_for_mode(item: &nclip_core::history::HistoryItem, as_: PasteAs) -> Vec<RawRep> {
        let filtered = as_.filter_reps(&item.reps);
        if !filtered.is_empty() {
            return filtered;
        }
        match as_ {
            // 경로만 — 평문 표현이 없는 파일 항목은 CF_HDROP에서 경로를 합성(09-01).
            PasteAs::PathOnly => {
                let paths: Vec<String> = item
                    .reps
                    .iter()
                    .filter(|r| r.format == "CF_HDROP")
                    .flat_map(|r| nclip_core::capture::parse_hdrop(&r.data))
                    .collect();
                if paths.is_empty() {
                    Vec::new()
                } else {
                    nclip_plat::clipboard::plain_text_reps(&paths.join("\r\n"))
                }
            }
            // ★ 평문 — PPT 글상자는 평문 표현이 아예 없다(09-02 사용자 요청) →
            //   표시와 같은 SVG <text> 추출을 CF_UNICODETEXT로 합성해 붙여넣는다.
            PasteAs::Plain => nclip_core::capture::svg_text(&item.reps)
                .map(|t| nclip_plat::clipboard::plain_text_reps(&t))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn copy_from_main(&mut self, id: u64, as_: PasteAs) {
        let Some(pos) =
            (0..self.history.len()).find(|&i| self.history.get(i).is_some_and(|it| it.id == id))
        else {
            return;
        };
        let Some(item) = self.history.get(pos) else {
            return;
        };
        let reps: Vec<RawRep> = Self::reps_for_mode(item, as_);
        if reps.is_empty() {
            eprintln!("평문 표현이 없는 항목입니다");
            return;
        }
        match nclip_plat::clipboard::set_reps(&reps) {
            Ok(n) => {
                println!("복사(메인창): \"{}\" — 표현 {n}개", item.label);
                self.history.expect_echo(pos);
            }
            Err(e) => eprintln!("복사 실패: {e}"),
        }
    }

    /// 항목을 클립보드로 되돌린다(트레이 메뉴 — 주입 없음).
    fn repost(&mut self, i: usize) {
        let Some(item) = self.history.get(i) else {
            return;
        };
        match nclip_plat::clipboard::set_reps(&item.reps) {
            Ok(n) => {
                println!("재적재: \"{}\" — 표현 {n}개 게시", item.label);
                // ★ 부분 게시의 에코는 원본 승격으로(08-30 Linux 실기 "같은 항목 둘").
                self.history.expect_echo(i);
            }
            Err(e) => eprintln!("재적재 실패: {e}"),
        }
    }

    /// ★ 팝업 선택 — 재적재 후 **기억해 둔 창으로 복원 + `Ctrl+V` 주입**(K-1 실물 경로).
    /// 4모드(T-15b)는 **클립보드 내용 선별**로 구현한다 — 주입 키는 항상 Ctrl+V.
    fn pick(&mut self, index: usize, as_: PasteAs) {
        let Some(item) = self.history.get(index) else {
            return;
        };
        let reps: Vec<RawRep> = Self::reps_for_mode(item, as_);
        if reps.is_empty() {
            // 그 모드가 무의미한 항목 — 있는 척하지 않는다(DR-31).
            eprintln!("이 항목에는 그 방식의 표현이 없습니다 — 원본(Enter)으로 붙여넣으세요");
            return;
        }
        let label = item.label.clone();
        self.popup.close();
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

    /// 팝업이 닫힌 다음 바퀴 — 포커스 복원 + 키 주입.
    fn paste_now(&mut self, as_: PasteAs) {
        match self.paste.restore_and_paste(as_) {
            Ok(()) => println!("붙여넣기: 포커스 복원 + 키 주입 ok"),
            Err(e) => eprintln!("붙여넣기 실패: {e:?} — 클립보드에는 실려 있습니다(Ctrl+V)"),
        }
    }

    /// 팝업 토글(전역 단축키) — 열 때 **먼저** 대상 포커스를 기억한다.
    fn toggle_popup(&mut self, el: &ActiveEventLoop) {
        if self.popup.is_open() {
            println!("팝업: 닫기(토글)");
            self.popup.close();
            return;
        }
        println!("팝업: 열기 — 이력 {}개", self.history.len());
        self.popup
            .set_view_code(self.app.conf.state.get("ui.popup_view"));
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
            match self.main.handle_event(&event) {
                MainAction::None => {}
                MainAction::Close => self.close_main(),
                MainAction::QueryChanged => self.main.on_history_changed(&self.history),
                MainAction::OpenSettings => self.app.ensure_window(el),
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
                    println!("최상위 고정: {}", if on { "켜짐" } else { "꺼짐" });
                }
                MainAction::TogglePreview => {
                    let on = self.app.conf.state.get("ui.preview_open") != "on";
                    self.app.conf.set(
                        "ui.preview_open",
                        if on { "on" } else { "off" }.to_string(),
                        Instant::now(),
                    );
                    self.main.apply_preview(on);
                }
                MainAction::Delete(id) => {
                    if self.history.remove(id) {
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
                        // ★ 핀 영속 = 같은 id Add 재기록(재생 시 교체 — docs/28 §4).
                        if let Some(it) = self.history.get_by_id(id) {
                            self.store.add(&to_stored(it));
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
                PopupAction::Close => self.popup.close(),
                PopupAction::Pick { index, as_ } => self.pick(index, as_),
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
                let geom = self.saved_main_geom();
                let view = self.app.conf.state.get("ui.view_mode").to_string();
                let atop = self.app.conf.state.get("ui.always_on_top") == "on";
                let pv = self.app.conf.state.get("ui.preview_open") == "on";
                self.main.open(
                    el,
                    &self.history,
                    theme,
                    geom,
                    crate::main_win::OpenOpts {
                        view_code: &view,
                        always_top: atop,
                        preview_open: pv,
                    },
                );
            }
            ShellEvent::OpenSettings => self.app.ensure_window(el),
            ShellEvent::Quit => {
                self.save_main_geom();
                el.exit();
            }
            ShellEvent::Hotkey => self.toggle_popup(el),
            ShellEvent::PasteAfterClose(as_) => self.paste_now(as_),
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
                    println!(
                        "전역 단축키: {} — 퀵 팝업",
                        nclip_plat::tray::hotkey_label()
                    );
                } else {
                    eprintln!(
                        "⚠️ 전역 단축키(Ctrl+Shift+V) 등록 실패 — {}. 트레이 좌클릭으로 여세요",
                        nclip_plat::tray::hotkey_failure_hint()
                    );
                }
            }
            ShellEvent::Captured(mut snap) => {
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
                // ★ 재적재로 되돌아온 우리 게시도 여기로 온다 — 승격(맨 위로)이
                //   곧 에코 처리다(항목이 늘지 않는다).
                let pushed: Pushed = self.history.push(&snap, kind, label, thumb);
                // ★ 영속(T-16) — 이력 변화를 그대로 이벤트로 흘린다(id가 짝이다).
                match pushed {
                    Pushed::New | Pushed::Replaced => {
                        if let Some(front) = self.history.get(0) {
                            self.store.add(&to_stored(front));
                        }
                    }
                    Pushed::Promoted => {
                        if let Some(front) = self.history.get(0) {
                            // ★ 승격이 램 섬네일을 채웠을 수 있다(무섬네일 세대 항목 재복사 —
                            //   09-02 실기: TOUCH만 남기면 재시작 후 텍스트로 퇴행). 섬네일이
                            //   있으면 ADD로 온전히 재기록 — 블롭은 내용 주소라 비용 미미.
                            if front.thumb.is_some() {
                                self.store.add(&to_stored(front));
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
                    // ★ 복사(중복 포함)가 들어오면 커서를 맨 위로 — 방금 것이
                    //   항상 첫 줄이고 선택도 그걸 가리킨다(08-28 사용자 요청).
                    self.popup.on_history_changed(&self.history);
                }
            }
            ShellEvent::Recent(i) => self.repost(i),
        }
        self.pump_preview();
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        ApplicationHandler::about_to_wait(&mut self.app, el);
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
            // ★ 스크롤바 자동 숨김 페이드(09-02) — 같은 박동에 얹는다.
            self.main.tick_ui(now_ms);
            self.popup.tick_ui(now_ms);
            let rem = 500 - (now_ms % 500);
            el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(rem.max(30)),
            ));
        }
    }
}

/// 트레이 + 감시 + 설정 창 상주. 종료는 트레이 메뉴에서.
pub(crate) fn run() {
    // ★ 설정을 먼저 — UI 글꼴(`ui.font_family`)이 폰트 선택을 좌우한다(09-01 "JetBrains Mono").
    let mut conf = Settings::load();
    crate::conf::apply_lang(&conf);
    let Some(font) = crate::conf::load_ui_font(&conf) else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다.");
        std::process::exit(1);
    };

    sync_autostart(&mut conf);

    let Ok(el) = EventLoop::<ShellEvent>::with_user_event().build() else {
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

    // 트레이 — 이벤트는 프록시로 메인 루프에 되돌린다(beep 호스트 문법).
    //   ★ 복원을 먼저 끝내 첫 우클릭부터 최근이 보인다(09-01 D1 — 예전엔 빈 메뉴로 떴다).
    let proxy = el.create_proxy();
    let Some(tray) = spawn(
        content(history.len(), history.recent_labels(tray_n)),
        move |ev| {
            let _ = proxy.send_event(match ev {
                TrayEvent::Quit => ShellEvent::Quit,
                TrayEvent::Recent(i) => ShellEvent::Recent(i),
                TrayEvent::Hotkey => ShellEvent::Hotkey,
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
        let token = crate::conf::data_dir().join("portal-remotedesktop.token");
        let _ = std::thread::Builder::new()
            .name("nclip-paste-warmup".into())
            .spawn(move || match nclip_plat::paste::warm_up(Some(token)) {
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
        println!("클립보드 감시: 이 OS는 미구현 — 트레이만 동작합니다");
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
    let mut shell = Shell {
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
        proxy: el.create_proxy(),
    };
    // ★ 복원 직후 트레이 메뉴·툴팁 갱신(09-01 사용자 실기 "우클릭에 최근이 안 보임") —
    //   spawn 때는 빈 내용이었고 첫 캡처까지는 아무도 불러주지 않았다.
    shell.refresh_tray();
    if let Err(e) = el.run_app(&mut shell) {
        eprintln!("이벤트 루프 오류: {e}");
    }
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
