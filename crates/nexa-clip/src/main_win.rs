//! S2 메인창 — ★ **클립보드 항목 관리**(T-18b0 · 설계 [docs/28](../../docs/28-main-window.md)).
//!
//! 퀵 팝업(S1)이 *골라 붙이는* 화면이라면, 여기는 **정리하는** 화면이다 —
//! CopyQ류의 관리 창을 우리 화법(자체 래스터 · [docs/04 §2-2] 확정 레이아웃)으로:
//! **검색 1줄 + 좌측 세로 툴바 40px + 목록(세로 최대) + 상태 1줄**.
//!
//! | 1단(지금) | 후속 |
//! |---|---|
//! | 검색 · ★핀 · 삭제 · 복사(원본/평문 재적재) · ⚙설정 | 메뉴바 · 편집(S4) · 태그 · 보기 3모드 · 가상화(T-18b) |
//!
//! 핀 항목은 **상단 구획**(최신순)이고 구분선 아래가 일반 항목이다 — 관리 화면의
//! 존재 이유("자주 쓰는 것이 안 떠내려간다"). 툴바 아이콘은 **벡터로 직접** 그린다
//! (글꼴 글리프는 두부 위험 — 09-01 `−` 교훈 · VT-1).

use nclip_core::capture::decode_plain;
use nclip_core::history::History;
use nclip_core::{ClipKind, PasteAs};
use nclip_ctl::controls::{ContextMenu, Control as _, CtxItem, TextBox};
use nclip_ctl::draw::{DrawCtx, FontSlot};
use nclip_ctl::event::{InputEvent as CtlEvent, Key as CtlKey};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
use nclip_ctl::widget::{Invalidations, Widget as _};
use nclip_ctl::ViewMode;
use nclip_gfx::{Font, Surface};

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// 시작 논리 크기 — 리사이즈 가능(관리 창).
const MAIN_W: f64 = 560.0;
const MAIN_H: f64 = 640.0;
/// 더블클릭 판정(ms).
const DBLCLICK_MS: u128 = 400;

/// 메인창에서 셸로 되돌리는 행동.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MainAction {
    /// 내부 처리 완료.
    None,
    /// 창 닫기 요청(X·Esc) — 숨김/종료 판단은 셸 몫(`ui.close_to_tray`).
    Close,
    /// ★ 항목을 클립보드로(원본/평문) — 주입 없음(관리 화면).
    Copy {
        /// 항목 id.
        id: u64,
        /// ★ 붙여넣기 방식(4모드 — T-15b · 클립보드 내용 선별).
        as_: PasteAs,
    },
    /// ★ 삭제 — 이력 + 저장소.
    Delete(u64),
    /// ★ 핀 토글.
    TogglePin(u64),
    /// 설정 창 열기(⚙).
    OpenSettings,
    /// ★ 보기 모드 변경(Ctrl+1/2/3) — 셸이 `ui.view_mode`에 영속한다.
    SetViewMode(&'static str),
    /// ★ 편집 저장(S4 평문화 · 09-01 확정).
    SaveEdit {
        /// 항목 id.
        id: u64,
        /// 새 평문 내용.
        text: String,
    },
    /// 검색어가 바뀌었다 — 셸이 이력으로 `refresh`를 다시 불러줘야 한다
    /// (창은 이력을 빌리지 않고 스냅샷만 든다).
    QueryChanged,
}

/// 목록 한 행(그리기용 사본 — 이력을 빌리지 않는다).
struct Row {
    id: u64,
    pinned: bool,
    kind: ClipKind,
    label: String,
    source: String,
    copies: u32,
    thumb: Option<nclip_ctl::theme::IconImage>,
    /// 첫 평문 표현(편집 시드 · Rich 보기 둘째 줄) — 없으면 None.
    plain: Option<String>,
}

/// 세로 툴바 버튼.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pin,
    Delete,
    Copy,
    CopyPlain,
    Settings,
}

/// 툴바 배치(위에서부터) — `None` = 구분선. ⚙는 바닥 고정(VT-4).
const TOOLS_TOP: [Option<Tool>; 5] = [
    Some(Tool::Pin),
    Some(Tool::Delete),
    None,
    Some(Tool::Copy),
    Some(Tool::CopyPlain),
];

pub(crate) struct MainWin {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    scale: f32,
    /// ★ 검색 입력(09-01 — 사용자 요청 "캐럿·복사/붙여넣기·드래그 선택") — 이식 `TextBox`
    ///   정식 편집기. 항상 포커스(키보드 기본 캡처)·IME preedit 인라인 표시.
    search: TextBox,
    rows: Vec<Row>,
    sel: usize,
    top: usize,
    cursor: (i32, i32),
    shift: bool,
    primary: bool,
    /// 더블클릭 판정 — (시각, 행).
    last_click: Option<(Instant, usize)>,
    /// 툴바 hover — 머티리얼 상태 레이어 + 툴팁(09-01 사용자 요청).
    hovered: Option<Tool>,
    /// ★ 보기 모드(Ctrl+1/2/3 · `ui.view_mode` 영속).
    view: ViewMode,
    /// ★ 우클릭 컨텍스트 메뉴(VT-5).
    menu: ContextMenu,
    /// ★ 인라인 편집(S4 평문화) — (항목 id, 멀티라인 입력).
    editor: Option<(u64, TextBox)>,
    /// 상태줄에 보일 전체 개수(필터 전).
    total: usize,
}

impl MainWin {
    pub(crate) fn new(font: Font) -> Self {
        Self {
            window: None,
            ctx: None,
            surface: None,
            font,
            theme: Theme::dark(),
            scale: 1.0,
            search: {
                let mut t = TextBox::new("검색…");
                t.set_focused(true);
                t
            },
            rows: Vec::new(),
            sel: 0,
            top: 0,
            cursor: (0, 0),
            shift: false,
            primary: false,
            last_click: None,
            hovered: None,
            view: ViewMode::Compact,
            menu: ContextMenu::new(),
            editor: None,
            total: 0,
        }
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    pub(crate) fn set_theme(&mut self, t: Theme) {
        if t.is_dark != self.theme.is_dark {
            self.theme = t;
            self.redraw();
        }
    }

    /// 창을 열거나 앞으로 가져온다(트레이 좌클릭·"열기").
    /// `geom` = 지난번 닫을 때 저장한 (x, y, w, h) 물리 좌표 — 같은 자리에 다시 연다(09-01).
    pub(crate) fn open(
        &mut self,
        el: &ActiveEventLoop,
        hist: &History,
        theme: Theme,
        geom: Option<(i32, i32, u32, u32)>,
        view_code: &str,
    ) {
        self.theme = theme;
        self.view = ViewMode::from_code(view_code).unwrap_or_default();
        if let Some(w) = &self.window {
            w.set_visible(true);
            w.focus_window();
            crate::settings_win::bring_to_front(w);
            self.refresh(hist);
            self.redraw();
            return;
        }
        self.search.set_text("");
        self.search.set_focused(true);
        self.sel = 0;
        self.top = 0;
        self.refresh(hist);
        let attrs = crate::settings_win::win_name(crate::icon::with_icon(
            Window::default_attributes()
                .with_title(if cfg!(target_os = "linux") {
                    "Nexa Clip"
                } else {
                    "Nexa Clip — 클립보드"
                })
                .with_inner_size(LogicalSize::new(MAIN_W, MAIN_H)),
        ));
        // ★ 마지막 위치·크기 복원 — 모니터 밖 좌표는 OS가 알아서 끌어온다(fail-soft).
        let attrs = match geom {
            Some((x, y, w, h)) => attrs
                .with_position(winit::dpi::PhysicalPosition::new(x, y))
                .with_inner_size(winit::dpi::PhysicalSize::new(w.max(200), h.max(200))),
            None => attrs,
        };
        let Ok(win) = el.create_window(attrs) else {
            eprintln!("메인창 생성 실패");
            return;
        };
        let win = Rc::new(win);
        crate::settings_win::bring_to_front(&win);
        self.scale = win.scale_factor() as f32;
        match softbuffer::Context::new(win.clone()) {
            Ok(ctx) => {
                match softbuffer::Surface::new(&ctx, win.clone()) {
                    Ok(s) => self.surface = Some(s),
                    Err(e) => eprintln!("softbuffer surface 실패: {e}"),
                }
                self.ctx = Some(ctx);
            }
            Err(e) => eprintln!("softbuffer context 실패: {e}"),
        }
        win.set_ime_allowed(true); // 한글 조합 중 검색(Preedit)
        win.focus_window();
        self.window = Some(win);
    }

    /// 현재 창 기하(x, y, w, h 물리) — 닫기 전에 저장해 다음 열기가 이어받는다.
    pub(crate) fn geometry(&self) -> Option<(i32, i32, u32, u32)> {
        let w = self.window.as_ref()?;
        let pos = w.outer_position().ok()?;
        let size = w.inner_size();
        Some((pos.x, pos.y, size.width, size.height))
    }

    /// 창만 걷는다(상주 유지) — 다음 open이 다시 만든다.
    pub(crate) fn close(&mut self) {
        self.surface = None;
        self.ctx = None;
        self.window = None;
    }

    /// 이력 → 행 스냅숏. ★ **핀 먼저(각 구획 최신순)** — 관리 화면의 정렬 계약.
    pub(crate) fn refresh(&mut self, hist: &History) {
        let q = self.search.display_text().to_lowercase();
        self.total = hist.len();
        let mut pinned = Vec::new();
        let mut normal = Vec::new();
        let mut i = 0usize;
        while let Some(item) = hist.get(i) {
            i += 1;
            if !q.is_empty() && !item.label.to_lowercase().contains(&q) {
                continue;
            }
            let row = Row {
                id: item.id,
                pinned: item.pinned,
                kind: item.kind,
                label: item.label.clone(),
                source: item.source_app.clone().unwrap_or_default(),
                copies: item.copies,
                thumb: item.thumb.as_ref().map(|(w, h, rgba)| {
                    nclip_ctl::theme::IconImage::from_rgba(*w, *h, rgba.clone())
                }),
                plain: item
                    .reps
                    .iter()
                    .find_map(|r| decode_plain(&r.format, &r.data)),
            };
            if row.pinned {
                pinned.push(row);
            } else {
                normal.push(row);
            }
        }
        self.rows = pinned;
        self.rows.extend(normal);
        if self.sel >= self.rows.len() {
            self.sel = self.rows.len().saturating_sub(1);
        }
        self.top = self.top.min(self.sel);
    }

    /// 이력이 바뀌었다(캡처·핀·삭제) — 열려 있으면 다시 채우고 그린다.
    pub(crate) fn on_history_changed(&mut self, hist: &History) {
        if self.window.is_some() {
            self.refresh(hist);
            self.redraw();
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn px(&self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }

    // ── 기하 ──

    fn toolbar_w(&self) -> i32 {
        self.px(40.0)
    }

    fn header_h(&self) -> i32 {
        self.px(38.0)
    }

    fn status_h(&self) -> i32 {
        self.px(22.0)
    }

    fn row_h(&self) -> i32 {
        match self.view {
            ViewMode::Rich => self.px(76.0).max(1),
            ViewMode::Compact => self.px(30.0).max(1),
            ViewMode::Plain => self.px(22.0).max(1),
        }
    }

    fn list_rect(&self, w: i32, h: i32) -> Rect {
        Rect::new(
            self.toolbar_w(),
            self.header_h(),
            (w - self.toolbar_w()).max(0),
            (h - self.header_h() - self.status_h()).max(0),
        )
    }

    fn tool_rect(&self, slot: usize) -> Rect {
        let side = self.px(28.0);
        let x = (self.toolbar_w() - side) / 2;
        let mut y = self.header_h() + self.px(6.0);
        for (k, t) in TOOLS_TOP.iter().enumerate() {
            if k == slot {
                break;
            }
            y += if t.is_some() {
                side + self.px(4.0)
            } else {
                self.px(9.0)
            };
        }
        Rect::new(x, y, side, side)
    }

    /// ⚙ — 바닥 고정(VT-4).
    fn settings_rect(&self, h: i32) -> Rect {
        let side = self.px(28.0);
        Rect::new(
            (self.toolbar_w() - side) / 2,
            h - self.status_h() - side - self.px(6.0),
            side,
            side,
        )
    }

    fn tool_at(&self, x: i32, y: i32, h: i32) -> Option<Tool> {
        if self.settings_rect(h).contains_xy(x, y) {
            return Some(Tool::Settings);
        }
        for (k, t) in TOOLS_TOP.iter().enumerate() {
            if let Some(t) = t {
                if self.tool_rect(k).contains_xy(x, y) {
                    return Some(*t);
                }
            }
        }
        None
    }

    fn row_at(&self, x: i32, y: i32, w: i32, h: i32) -> Option<usize> {
        let l = self.list_rect(w, h);
        if x < l.x || x >= l.x + l.w || y < l.y || y >= l.y + l.h {
            return None;
        }
        let vi = self.top + ((y - l.y) / self.row_h()) as usize;
        (vi < self.rows.len()).then_some(vi)
    }

    fn selected_id(&self) -> Option<u64> {
        self.rows.get(self.sel).map(|r| r.id)
    }

    fn act(&self, tool: Tool) -> MainAction {
        match (tool, self.selected_id()) {
            (Tool::Settings, _) => MainAction::OpenSettings,
            (_, None) => MainAction::None, // VT-3: 선택 없으면 비활성
            (Tool::Pin, Some(id)) => MainAction::TogglePin(id),
            (Tool::Delete, Some(id)) => MainAction::Delete(id),
            (Tool::Copy, Some(id)) => MainAction::Copy {
                id,
                as_: PasteAs::Original,
            },
            (Tool::CopyPlain, Some(id)) => MainAction::Copy {
                id,
                as_: PasteAs::Plain,
            },
        }
    }

    /// 창 이벤트 처리 — 행동은 셸로 되돌린다.
    pub(crate) fn handle_event(&mut self, event: &WindowEvent) -> MainAction {
        let (w, h) = match &self.window {
            Some(win) => {
                let s = win.inner_size();
                (s.width as i32, s.height as i32)
            }
            None => return MainAction::None,
        };
        // ★ 열린 컨텍스트 메뉴가 있으면 먼저 먹는다(바깥 클릭 = 닫기 — 메뉴 계약).
        if self.menu.is_open() {
            if let Some(ev) = to_ctl_event(event, self.cursor) {
                let changed = self.menu.on_event(&ev);
                if changed {
                    self.redraw();
                }
                if let Some(id) = self.menu.take_picked() {
                    return self.menu_action(&id);
                }
                // 메뉴가 열려 있는 동안 메인 입력은 전부 메뉴 몸.
                if matches!(
                    ev,
                    CtlEvent::MouseDown { .. }
                        | CtlEvent::MouseUp { .. }
                        | CtlEvent::Key { .. }
                        | CtlEvent::Wheel { .. }
                ) {
                    return MainAction::None;
                }
            }
            if matches!(event, WindowEvent::RedrawRequested) {
                self.paint();
            }
            if let WindowEvent::CursorMoved { position, .. } = event {
                self.cursor = (position.x as i32, position.y as i32);
            }
            return MainAction::None;
        }
        // ★ 인라인 편집 중 — Esc 취소 · Ctrl+Enter 저장 · 나머지는 입력 상자로.
        if self.editor.is_some() {
            return self.handle_editor_event(event);
        }
        match event {
            WindowEvent::CloseRequested => return MainAction::Close,
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = *scale_factor as f32;
                self.redraw();
            }
            WindowEvent::Resized(_) => self.redraw(),
            WindowEvent::Ime(ime) => {
                use winit::event::Ime;
                let mut inv = Invalidations::default();
                let changed = match ime {
                    Ime::Preedit(t, _) => {
                        self.search.set_preedit(t, &mut inv);
                        true
                    }
                    Ime::Commit(t) => {
                        self.search.set_preedit("", &mut inv);
                        for c in t.chars().filter(|c| !c.is_control()) {
                            self.search
                                .on_event(&CtlEvent::Char { c, now_ms: 0 }, &mut inv);
                        }
                        true
                    }
                    _ => false,
                };
                if changed {
                    return MainAction::QueryChanged;
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.primary = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                // 드래그 선택 추적 — 캡처 없이 흘려도 TextBox가 자기 상태로 판단한다.
                let mut inv = Invalidations::default();
                self.search.on_event(
                    &CtlEvent::MouseMove {
                        x: self.cursor.0,
                        y: self.cursor.1,
                    },
                    &mut inv,
                );
                if !inv.is_empty() {
                    self.redraw();
                }
                let hovered = self.tool_at(self.cursor.0, self.cursor.1, h);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if self.hovered.take().is_some() {
                    self.redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let step = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -*y as i32 * 3,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as i32) / 20,
                };
                if step != 0 && !self.rows.is_empty() {
                    let max_top = self.rows.len() - 1;
                    self.top = self.top.saturating_add_signed(step as isize).min(max_top);
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Pressed && *button == winit::event::MouseButton::Right {
                    let (x, y) = self.cursor;
                    if let Some(vi) = self.row_at(x, y, w, h) {
                        self.sel = vi;
                        self.open_menu(x, y, w, h);
                        self.redraw();
                    }
                    return MainAction::None;
                }
                if *state == ElementState::Released && *button == winit::event::MouseButton::Left {
                    let (x, y) = self.cursor;
                    let mut inv = Invalidations::default();
                    self.search.on_event(&CtlEvent::MouseUp { x, y }, &mut inv);
                }
                if *state == ElementState::Pressed && *button == winit::event::MouseButton::Left {
                    let (x, y) = self.cursor;
                    if self
                        .search
                        .bounds()
                        .contains(nclip_ctl::geom::Point { x, y })
                    {
                        let mut inv = Invalidations::default();
                        self.search.on_event(
                            &CtlEvent::MouseDown {
                                x,
                                y,
                                shift: self.shift,
                                primary: self.primary,
                            },
                            &mut inv,
                        );
                        self.redraw();
                        return MainAction::None;
                    }
                    if let Some(t) = self.tool_at(x, y, h) {
                        return self.act(t);
                    }
                    if let Some(vi) = self.row_at(x, y, w, h) {
                        let now = Instant::now();
                        let dbl = self
                            .last_click
                            .is_some_and(|(t, r)| r == vi && t.elapsed().as_millis() < DBLCLICK_MS);
                        self.last_click = Some((now, vi));
                        self.sel = vi;
                        self.redraw();
                        if dbl {
                            if let Some(id) = self.selected_id() {
                                return MainAction::Copy {
                                    id,
                                    as_: if self.shift {
                                        PasteAs::Plain
                                    } else {
                                        PasteAs::Original
                                    },
                                };
                            }
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return MainAction::None;
                }
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => return MainAction::Close,
                    Key::Named(NamedKey::ArrowUp) => {
                        self.sel = self.sel.saturating_sub(1);
                        self.ensure_visible(h);
                        self.redraw();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.sel + 1 < self.rows.len() {
                            self.sel += 1;
                        }
                        self.ensure_visible(h);
                        self.redraw();
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some(id) = self.selected_id() {
                            return MainAction::Copy {
                                id,
                                as_: if self.shift {
                                    PasteAs::Plain
                                } else {
                                    PasteAs::Original
                                },
                            };
                        }
                    }
                    Key::Named(NamedKey::Delete) => {
                        if let Some(id) = self.selected_id() {
                            return MainAction::Delete(id);
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        let mut inv = Invalidations::default();
                        self.search.on_event(
                            &CtlEvent::Char {
                                c: '\u{8}',
                                now_ms: 0,
                            },
                            &mut inv,
                        );
                        return MainAction::QueryChanged;
                    }
                    // ★ 캐럿 이동·범위 선택(⇧) — 검색 입력의 표준 편집 키(09-01).
                    Key::Named(
                        k @ (NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                        | NamedKey::Home
                        | NamedKey::End),
                    ) => {
                        let key = match k {
                            NamedKey::ArrowLeft => CtlKey::Left,
                            NamedKey::ArrowRight => CtlKey::Right,
                            NamedKey::Home => CtlKey::Home,
                            _ => CtlKey::End,
                        };
                        let mut inv = Invalidations::default();
                        self.search.on_event(
                            &CtlEvent::Key {
                                key,
                                shift: self.shift,
                                primary: self.primary,
                            },
                            &mut inv,
                        );
                        if !inv.is_empty() {
                            self.redraw();
                        }
                    }
                    // ★ 클립보드 단축(09-01) — 전체 선택·복사·잘라내기·붙여넣기.
                    Key::Character("a" | "A") if self.primary => {
                        let mut inv = Invalidations::default();
                        self.search.on_event(&CtlEvent::SelectAll, &mut inv);
                        self.redraw();
                    }
                    Key::Character("c" | "C") if self.primary => {
                        if let Some(t) = self.search.copy_selection() {
                            let _ = nclip_plat::clipboard::set_reps(
                                &nclip_plat::clipboard::plain_text_reps(&t),
                            );
                        }
                    }
                    Key::Character("x" | "X") if self.primary => {
                        let mut inv = Invalidations::default();
                        if let Some(t) = self.search.cut_selection(&mut inv) {
                            let _ = nclip_plat::clipboard::set_reps(
                                &nclip_plat::clipboard::plain_text_reps(&t),
                            );
                            return MainAction::QueryChanged;
                        }
                    }
                    Key::Character("v" | "V") if self.primary => {
                        let text = nclip_plat::watch::PlatformWatch::new()
                            .read_now()
                            .and_then(|s| s.plain_text());
                        if let Some(t) = text {
                            let mut inv = Invalidations::default();
                            self.search.paste(t.trim_end_matches('\n'), &mut inv);
                            return MainAction::QueryChanged;
                        }
                    }
                    // ★ 보기 3모드(Ctrl+1/2/3 — docs/04 §2-2 보기 메뉴 계약).
                    Key::Character(d @ ("1" | "2" | "3")) if self.primary => {
                        let (v, code) = match d {
                            "1" => (ViewMode::Rich, "rich"),
                            "2" => (ViewMode::Compact, "compact"),
                            _ => (ViewMode::Plain, "plain"),
                        };
                        if self.view != v {
                            self.view = v;
                            self.redraw();
                            return MainAction::SetViewMode(code);
                        }
                    }
                    Key::Character("p" | "P") if self.primary => {
                        if let Some(id) = self.selected_id() {
                            return MainAction::TogglePin(id);
                        }
                    }
                    Key::Character(t) if !self.primary => {
                        let mut changed = false;
                        let mut inv = Invalidations::default();
                        for c in t.chars().filter(|c| !c.is_control()) {
                            self.search
                                .on_event(&CtlEvent::Char { c, now_ms: 0 }, &mut inv);
                            changed = true;
                        }
                        if changed {
                            return MainAction::QueryChanged;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        MainAction::None
    }

    fn ensure_visible(&mut self, h: i32) {
        let l_h = (h - self.header_h() - self.status_h()).max(1);
        let visible = (l_h / self.row_h()).max(1) as usize;
        if self.sel < self.top {
            self.top = self.sel;
        } else if self.sel >= self.top + visible {
            self.top = self.sel + 1 - visible;
        }
    }

    fn paint(&mut self) {
        let Some(win) = self.window.clone() else {
            return;
        };
        let size = win.inner_size();
        let (Some(nw), Some(nh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        let (w, h) = (size.width as i32, size.height as i32);
        self.ensure_visible(h);
        {
            let pad = (self.scale * 10.0).round() as i32;
            let mut inv = Invalidations::default();
            self.search.set_scale(self.scale);
            self.search.set_bounds(
                Rect::new(
                    pad,
                    (self.scale * 7.0).round() as i32,
                    (w - pad * 2).max(40),
                    (self.scale * 24.0).round() as i32,
                ),
                &mut inv,
            );
        }
        if let Some((_, tb)) = self.editor.as_mut() {
            let list = {
                let tbw = (self.scale * 40.0).round() as i32;
                let hh = (self.scale * 38.0).round() as i32;
                let sh = (self.scale * 22.0).round() as i32;
                let pad = (self.scale * 10.0).round() as i32;
                Rect::new(
                    tbw + pad,
                    hh + pad,
                    (w - tbw - pad * 2).max(60),
                    (h - hh - sh - pad * 2 - (self.scale * 20.0) as i32).max(60),
                )
            };
            let mut inv = Invalidations::default();
            tb.set_bounds(list, &mut inv);
        }
        // ★ surface를 잠시 꺼내 빌림을 끕는다 — draw(&self)가 전체 상태를 읽기 때문.
        let Some(mut surface) = self.surface.take() else {
            return;
        };
        if surface.resize(nw, nh).is_ok() {
            if let Ok(mut buf) = surface.buffer_mut() {
                {
                    let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
                    let mut dc = RasterCtx::new(&mut gfx, &self.font, self.scale);
                    self.draw(&mut dc, w, h);
                }
                let _ = buf.present();
            }
        }
        self.surface = Some(surface);
    }

    #[allow(clippy::too_many_lines)]
    fn draw(&self, dc: &mut RasterCtx<'_, '_, '_>, w: i32, h: i32) {
        let th = self.theme;
        let px = |v: f32| (v * self.scale).round() as i32;
        let full = Rect::new(0, 0, w, h);
        dc.select_font(FontSlot::Base, false);
        dc.fill_rect(full, th.window_bg);

        // ── ① 검색 1줄 — 정식 TextBox(캐럿·선택 표시 · 09-01) ──
        let header_h = self.header_h();
        dc.fill_rect(Rect::new(0, 0, w, header_h), th.chrome_bg);
        let pad = px(10.0);
        self.search.paint(dc, &th);
        dc.fill_rect(Rect::new(0, header_h - 1, w, 1), th.border);

        // ── ② 좌측 세로 툴바 ──
        let tb_w = self.toolbar_w();
        dc.fill_rect(
            Rect::new(0, header_h, tb_w, h - header_h - self.status_h()),
            th.chrome_bg,
        );
        dc.fill_rect(
            Rect::new(tb_w - 1, header_h, 1, h - header_h - self.status_h()),
            th.border,
        );
        let has_sel = !self.rows.is_empty();
        for (k, t) in TOOLS_TOP.iter().enumerate() {
            match t {
                Some(t) => self.draw_tool(dc, self.tool_rect(k), *t, has_sel),
                None => {
                    let r = self.tool_rect(k);
                    dc.fill_rect(
                        Rect::new(r.x + px(4.0), r.y + px(3.0), r.w - px(8.0), 1),
                        th.border,
                    );
                }
            }
        }
        self.draw_tool(dc, self.settings_rect(h), Tool::Settings, true);

        // ── ③ 목록(핀 구획 먼저) ──
        let list = self.list_rect(w, h);
        let row_h = self.row_h();
        let visible = (list.h / row_h).max(1) as usize;
        if self.rows.is_empty() {
            let msg = if self.search.display_text().is_empty() {
                "항목이 없습니다 — 복사하면 여기 쌓입니다"
            } else {
                "일치하는 항목이 없습니다"
            };
            dc.text(list.x + pad, list.y + px(12.0), full, msg, th.text_dim);
        }
        let mut pin_divider_done = false;
        for (vi, row) in self.rows.iter().enumerate().skip(self.top).take(visible) {
            let y = list.y + ((vi - self.top) as i32) * row_h;
            let clip = Rect::new(list.x, y, list.w, row_h.min(list.y + list.h - y));
            if vi == self.sel {
                dc.fill_rect(clip, th.sel_bg);
            } else if vi % 2 == 1 {
                dc.fill_rect(clip, th.panel_bg_alt);
            }
            // 핀 구획 경계 — 첫 비고정 행 위에 한 줄.
            if !pin_divider_done && !row.pinned && vi > 0 {
                dc.fill_rect(Rect::new(list.x, y, list.w, 1), th.accent);
                pin_divider_done = true;
            }
            let tx = list.x + pad;
            // ★ Rich = 행 번호(CopyQ 화법 · 09-01 J4) — 목록 위치가 곧 번호(1 = 최신).
            if self.view == ViewMode::Rich {
                dc.select_font(FontSlot::Status, false);
                let no = format!("{}", vi + 1);
                dc.text(tx, y + px(6.0), clip, &no, th.text_dim);
                dc.select_font(FontSlot::Base, false);
            }
            // 보기 3모드(docs/04 §2-2) — Plain은 글리프도 접어 밀도 최우선.
            let show_glyph = self.view != ViewMode::Plain;
            if show_glyph {
                if let Some(img) = &row.thumb {
                    let box_side = if self.view == ViewMode::Rich {
                        px(40.0)
                    } else {
                        px(24.0)
                    };
                    let (iw, ih) = (img.w.max(1) as i32, img.h.max(1) as i32);
                    let (dw, dh) = if iw >= ih {
                        (box_side, (box_side * ih / iw).max(1))
                    } else {
                        ((box_side * iw / ih).max(1), box_side)
                    };
                    let dst = Rect::new(tx + (box_side - dw) / 2, y + (row_h - dh) / 2, dw, dh);
                    dc.image_scaled(dst, img, clip);
                } else {
                    dc.text(
                        tx,
                        y + (row_h - px(16.0)) / 2,
                        clip,
                        crate::popup_win::kind_glyph(row.kind),
                        th.accent,
                    );
                }
            }
            // 핀 표식 — 라벨 앞 작은 점.
            let text_y = match self.view {
                ViewMode::Rich => y + px(7.0),
                ViewMode::Compact => y + px(7.0),
                ViewMode::Plain => y + px(3.0),
            };
            let mut lx = tx
                + if self.view == ViewMode::Plain {
                    px(0.0)
                } else if self.view == ViewMode::Rich {
                    px(48.0)
                } else {
                    px(30.0)
                };
            if row.pinned {
                dc.fill_round_rect(
                    Rect::new(lx, text_y + px(4.0), px(6.0), px(6.0)),
                    px(3.0),
                    th.accent,
                );
                lx += px(12.0);
            }
            // 우측 메타(출처 · ×n) 먼저 재서 라벨 clip을 줄인다.
            let mut right = list.x + list.w - pad;
            if row.copies > 1 {
                let tag = format!("×{}", row.copies);
                let tw = dc.text_width(&tag);
                right -= tw;
                dc.text(right, text_y, clip, &tag, th.text_dim);
                right -= px(8.0);
            }
            // 출처 — Rich는 둘째 줄로 내려가 라벨이 전폭을 쓴다.
            if !row.source.is_empty() && self.view != ViewMode::Rich {
                let tw = dc.text_width(&row.source);
                right -= tw;
                dc.text(right, text_y, clip, &row.source, th.text_dim);
                right -= px(8.0);
            }
            let label_clip = Rect::new(lx, y, (right - lx).max(0), row_h);
            dc.text(lx, text_y, label_clip, &row.label, th.text);
            if self.view == ViewMode::Rich {
                dc.select_font(FontSlot::Status, false);
                // 둘째 줄 = 출처 · 아래 두 줄 = 본문 미리보기(개행 유지 — CopyQ 화법).
                if !row.source.is_empty() {
                    dc.text(lx, y + px(24.0), label_clip, &row.source, th.text_dim);
                }
                if let Some(pl) = &row.plain {
                    for (k, line) in pl.lines().take(2).enumerate() {
                        let one: String = line.chars().take(140).collect();
                        #[allow(clippy::cast_precision_loss)]
                        dc.text(
                            lx,
                            y + px(40.0 + 16.0 * k as f32),
                            label_clip,
                            &one,
                            th.text_dim,
                        );
                    }
                }
                dc.select_font(FontSlot::Base, false);
            }
        }

        // ── ④ 상태 1줄 ──
        let sy = h - self.status_h();
        dc.fill_rect(Rect::new(0, sy, w, self.status_h()), th.chrome_bg);
        dc.fill_rect(Rect::new(0, sy, w, 1), th.border);
        let status = if self.search.display_text().is_empty() {
            format!("{}개 · 암호화 · 로컬", self.total)
        } else {
            format!("{} / {}개 · 암호화 · 로컬", self.rows.len(), self.total)
        };
        dc.select_font(FontSlot::Status, false);
        dc.text(pad, sy + px(4.0), full, &status, th.text_dim);
        dc.select_font(FontSlot::Base, false);

        // ★ 툴팁 — **반드시 맨 끝**(09-01 실기 "일부만 보임" = 목록이 덤어버렸다).
        //   글자는 본문 크기(Base) — Status는 작다는 사용자 피드백.
        if let Some(t) = self.hovered {
            let r = self.tool_rect_of(t, h);
            let label = tool_label(t);
            let tw = dc.text_width(label);
            let (tip_h, pad_x) = (px(26.0), px(10.0));
            let tip = Rect::new(
                r.x + r.w + px(6.0),
                r.y + (r.h - tip_h) / 2,
                tw + pad_x * 2,
                tip_h,
            );
            dc.fill_round_rect(tip, px(5.0), th.chrome_bg);
            dc.stroke_round_rect(tip, px(5.0), th.border, 1.0);
            dc.text(tip.x + pad_x, tip.y + px(5.0), full, label, th.text);
        }

        // ★ 인라인 에디터(S4 평문화) — 목록 영역을 덮는 시트 + 안내 줄.
        if let Some((_, tb)) = &self.editor {
            let list = self.list_rect(w, h);
            dc.fill_rect(list, th.panel_bg);
            tb.paint(dc, &th);
            dc.select_font(FontSlot::Status, false);
            dc.text(
                list.x + px(10.0),
                list.y + list.h - px(18.0),
                full,
                "Ctrl+Enter 저장 · Esc 취소 (평문으로 저장됩니다)",
                th.text_dim,
            );
            dc.select_font(FontSlot::Base, false);
        }
        // ★ 컨텍스트 메뉴 — 언제나 맨 위(툴팁 교훈).
        if self.menu.is_open() {
            self.menu.paint(dc, &th);
        }
    }

    /// 툴바 버튼의 rect — hover 툴팁이 자리를 되찾는다.
    fn tool_rect_of(&self, tool: Tool, h: i32) -> Rect {
        if tool == Tool::Settings {
            return self.settings_rect(h);
        }
        for (k, t) in TOOLS_TOP.iter().enumerate() {
            if *t == Some(tool) {
                return self.tool_rect(k);
            }
        }
        Rect::default()
    }

    /// 툴바 아이콘 — 전부 벡터(사각·원 조합 · 구글 머티리얼 아이콘 버튼 문법:
    /// hover = 원형 상태 레이어 · 아이콘은 2px 선). `enabled=false`면 흐리게(VT-3).
    fn draw_tool(&self, dc: &mut RasterCtx<'_, '_, '_>, r: Rect, tool: Tool, enabled: bool) {
        let th = self.theme;
        // ★ 상태 레이어(Material 3) — hover한 활성 버튼에만 은은한 원.
        if enabled && self.hovered == Some(tool) {
            dc.fill_round_rect_alpha(r, r.w / 2, th.text, 0.10);
        }
        let ink = if enabled { th.text } else { th.text_dim };
        let px = |v: f32| (v * self.scale).round() as i32;
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        // ★ 20px 박스(사용자 확정 09-01) · Material Symbols 형상을 사각/원 조합으로 근사 —
        //   글꼴 링크 없이 단일 바이너리 유지(DR-8). 선 굵기 ≈ 1.7px(Material 400 가중).
        let w2 = 1.7f32 * self.scale;
        match tool {
            Tool::Pin => {
                // Material `keep`(push pin) — 위 깔땏기 몸통 + 받침 널판 + 바늘.
                dc.fill_round_rect(
                    Rect::new(cx - px(3.5), cy - px(9.0), px(7.0), px(9.0)),
                    px(1.5),
                    ink,
                );
                dc.fill_round_rect(
                    Rect::new(cx - px(6.0), cy - px(1.0), px(12.0), px(2.5)),
                    px(1.0),
                    ink,
                );
                dc.fill_rect(
                    Rect::new(cx - 1, cy + px(1.5), px(2.0).max(2), px(7.0)),
                    ink,
                );
            }
            Tool::Delete => {
                // Material `delete` — 손잡이·둪꺼워·몸통(세로줄 2).
                dc.fill_round_rect(
                    Rect::new(cx - px(2.5), cy - px(9.5), px(5.0), px(2.0)),
                    px(1.0),
                    ink,
                );
                dc.fill_round_rect(
                    Rect::new(cx - px(7.0), cy - px(8.0), px(14.0), px(2.0)),
                    px(1.0),
                    ink,
                );
                dc.stroke_round_rect(
                    Rect::new(cx - px(5.5), cy - px(5.0), px(11.0), px(14.0)),
                    px(2.0),
                    ink,
                    w2,
                );
                dc.fill_rect(
                    Rect::new(cx - px(2.0), cy - px(2.0), px(1.5).max(2), px(8.0)),
                    ink,
                );
                dc.fill_rect(
                    Rect::new(cx + px(1.0), cy - px(2.0), px(1.5).max(2), px(8.0)),
                    ink,
                );
            }
            Tool::Copy => {
                // Material `content_copy` — 뒤장(왼위) + 앞장(오른아래 · 속을 바닥색으로 가려 겹침 표현).
                dc.stroke_round_rect(
                    Rect::new(cx - px(9.0), cy - px(9.0), px(12.0), px(14.0)),
                    px(2.0),
                    ink,
                    w2,
                );
                let front = Rect::new(cx - px(4.0), cy - px(4.5), px(12.0), px(14.0));
                dc.fill_round_rect(front, px(2.0), th.chrome_bg);
                dc.stroke_round_rect(front, px(2.0), ink, w2);
            }
            Tool::CopyPlain => {
                // Material `text_snippet` — 문서 테두리 + 글줄 3(마지막은 짧게).
                dc.stroke_round_rect(
                    Rect::new(cx - px(8.0), cy - px(8.0), px(16.0), px(16.0)),
                    px(2.5),
                    ink,
                    w2,
                );
                dc.fill_rect(
                    Rect::new(cx - px(4.5), cy - px(4.0), px(9.0), px(1.8).max(2)),
                    ink,
                );
                dc.fill_rect(
                    Rect::new(cx - px(4.5), cy - px(0.9), px(9.0), px(1.8).max(2)),
                    ink,
                );
                dc.fill_rect(
                    Rect::new(cx - px(4.5), cy + px(2.2), px(5.0), px(1.8).max(2)),
                    ink,
                );
            }
            Tool::Settings => {
                // Material `settings` — 톱니 8개(4방 + 대각) + 링 + 중심 구멍.
                let ring = Rect::new(cx - px(6.5), cy - px(6.5), px(13.0), px(13.0));
                for (dx, dy, ww, hh) in [
                    (-px(1.5), -px(10.0), px(3.0), px(4.0)),
                    (-px(1.5), px(6.0), px(3.0), px(4.0)),
                    (-px(10.0), -px(1.5), px(4.0), px(3.0)),
                    (px(6.0), -px(1.5), px(4.0), px(3.0)),
                ] {
                    dc.fill_round_rect(Rect::new(cx + dx, cy + dy, ww, hh), px(1.0), ink);
                }
                for (dx, dy) in [
                    (-px(7.5), -px(7.5)),
                    (px(4.5), -px(7.5)),
                    (-px(7.5), px(4.5)),
                    (px(4.5), px(4.5)),
                ] {
                    dc.fill_round_rect(Rect::new(cx + dx, cy + dy, px(3.0), px(3.0)), px(1.0), ink);
                }
                dc.fill_ellipse(ring, ink);
                let hole = Rect::new(cx - px(2.8), cy - px(2.8), px(5.6), px(5.6));
                dc.fill_ellipse(hole, th.chrome_bg);
            }
        }
    }
}

impl MainWin {
    /// 우클릭 메뉴 — 툴바와 같은 항목 전부(VT-5) + 파일은 개체/경로(4모드).
    fn open_menu(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let Some(row) = self.rows.get(self.sel) else {
            return;
        };
        let mut items = vec![
            CtxItem::item("copy", "복사"),
            CtxItem::item("plain", "평문으로 복사"),
        ];
        if row.kind == ClipKind::Files {
            items.push(CtxItem::item("object", "개체로 복사"));
            items.push(CtxItem::item("path", "경로만 복사"));
        }
        items.push(CtxItem::item(
            "pin",
            if row.pinned {
                "고정 해제"
            } else {
                "고정"
            },
        ));
        let editable = matches!(row.kind, ClipKind::Text | ClipKind::RichText);
        items.push(CtxItem::maybe("edit", "편집(평문화)", editable));
        items.push(CtxItem::item("delete", "삭제"));
        self.menu.set_scale(self.scale);
        // 라벨 폭 추정 — 페인트 전이라 실측 불가(한글 13px 기준 넉넉하게).
        let text_w = self.px(13.0) * 8;
        self.menu
            .open_at(x, y, items, Rect::new(0, 0, w, h), text_w);
    }

    /// 메뉴 선택 id → 셸 행동.
    fn menu_action(&mut self, id: &str) -> MainAction {
        let Some(item_id) = self.selected_id() else {
            return MainAction::None;
        };
        match id {
            "copy" => MainAction::Copy {
                id: item_id,
                as_: PasteAs::Original,
            },
            "plain" => MainAction::Copy {
                id: item_id,
                as_: PasteAs::Plain,
            },
            "object" => MainAction::Copy {
                id: item_id,
                as_: PasteAs::Object,
            },
            "path" => MainAction::Copy {
                id: item_id,
                as_: PasteAs::PathOnly,
            },
            "pin" => MainAction::TogglePin(item_id),
            "delete" => MainAction::Delete(item_id),
            "edit" => {
                self.begin_edit(item_id);
                MainAction::None
            }
            _ => MainAction::None,
        }
    }

    /// ★ 편집 시작(S4 평문화) — 첫 평문 표현을 멀티라인 입력으로.
    fn begin_edit(&mut self, id: u64) {
        let Some(row) = self.rows.iter().find(|r| r.id == id) else {
            return;
        };
        let text = row.plain.clone().unwrap_or_default();
        let mut tb = TextBox::new("").with_multiline().with_text(&text);
        tb.set_scale(self.scale);
        tb.set_focused(true);
        self.editor = Some((id, tb));
        self.redraw();
    }

    /// 편집 중 이벤트 — Esc 취소 · Ctrl+Enter 저장 · 나머지는 입력 상자로.
    fn handle_editor_event(&mut self, event: &WindowEvent) -> MainAction {
        match event {
            WindowEvent::CloseRequested => {
                self.editor = None;
                return MainAction::Close;
            }
            WindowEvent::RedrawRequested => {
                self.paint();
                return MainAction::None;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = *scale_factor as f32;
                self.redraw();
                return MainAction::None;
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.primary = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
                return MainAction::None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
            }
            // ★ 한글 입력(09-01 J5) — IME 조합은 Char가 아니라 Ime 이벤트로 온다.
            WindowEvent::Ime(ime) => {
                use winit::event::Ime;
                if let Some((_, tb)) = self.editor.as_mut() {
                    let mut inv = Invalidations::default();
                    match ime {
                        Ime::Preedit(t, _) => tb.set_preedit(t, &mut inv),
                        Ime::Commit(t) => {
                            tb.set_preedit("", &mut inv);
                            for c in t.chars().filter(|c| !c.is_control()) {
                                tb.on_event(&CtlEvent::Char { c, now_ms: 0 }, &mut inv);
                            }
                        }
                        _ => {}
                    }
                    self.redraw();
                }
                return MainAction::None;
            }
            WindowEvent::KeyboardInput { event: kev, .. } if kev.state == ElementState::Pressed => {
                match kev.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.editor = None;
                        self.redraw();
                        return MainAction::None;
                    }
                    Key::Named(NamedKey::Enter) if self.primary => {
                        if let Some((id, tb)) = self.editor.take() {
                            self.redraw();
                            return MainAction::SaveEdit {
                                id,
                                text: tb.text(),
                            };
                        }
                        return MainAction::None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // 나머지 입력은 입력 상자로(변환 가능한 것만).
        if let Some(ev) = to_ctl_event(event, self.cursor) {
            if let Some((_, tb)) = self.editor.as_mut() {
                let mut inv = Invalidations::default();
                tb.on_event(&ev, &mut inv);
                if !inv.is_empty() {
                    self.redraw();
                }
            }
        }
        MainAction::None
    }
}

/// winit 이벤트 → nclip-ctl [`CtlEvent`] 최소 변환(메뉴·입력 상자용) —
/// 메인창은 winit을 직접 다루지만 이식 컨트롤은 ctl 이벤트를 말한다.
fn to_ctl_event(event: &WindowEvent, cursor: (i32, i32)) -> Option<CtlEvent> {
    let (x, y) = cursor;
    let key = |key: CtlKey| CtlEvent::Key {
        key,
        shift: false,
        primary: false,
    };
    Some(match event {
        WindowEvent::CursorMoved { position, .. } => CtlEvent::MouseMove {
            x: position.x as i32,
            y: position.y as i32,
        },
        WindowEvent::MouseInput { state, button, .. } => match (state, button) {
            (ElementState::Pressed, winit::event::MouseButton::Left) => CtlEvent::MouseDown {
                x,
                y,
                shift: false,
                primary: false,
            },
            (ElementState::Released, winit::event::MouseButton::Left) => CtlEvent::MouseUp { x, y },
            (ElementState::Pressed, winit::event::MouseButton::Right) => {
                CtlEvent::RightDown { x, y }
            }
            _ => return None,
        },
        WindowEvent::MouseWheel { delta, .. } => CtlEvent::Wheel {
            delta: match delta {
                MouseScrollDelta::LineDelta(_, dy) => (*dy * 120.0) as i32,
                MouseScrollDelta::PixelDelta(p) => p.y as i32,
            },
        },
        WindowEvent::KeyboardInput { event: kev, .. } if kev.state == ElementState::Pressed => {
            match kev.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => key(CtlKey::Enter),
                Key::Named(NamedKey::Escape) => key(CtlKey::Escape),
                Key::Named(NamedKey::ArrowUp) => key(CtlKey::Up),
                Key::Named(NamedKey::ArrowDown) => key(CtlKey::Down),
                Key::Named(NamedKey::ArrowLeft) => key(CtlKey::Left),
                Key::Named(NamedKey::ArrowRight) => key(CtlKey::Right),
                Key::Named(NamedKey::Home) => key(CtlKey::Home),
                Key::Named(NamedKey::End) => key(CtlKey::End),
                Key::Named(NamedKey::Delete) => key(CtlKey::Delete),
                Key::Named(NamedKey::Backspace) => CtlEvent::Char {
                    c: '\u{8}',
                    now_ms: 0,
                },
                Key::Named(NamedKey::Space) => CtlEvent::Char { c: ' ', now_ms: 0 },
                Key::Character(t) => {
                    let c = t.chars().next()?;
                    if c.is_control() {
                        return None;
                    }
                    CtlEvent::Char { c, now_ms: 0 }
                }
                _ => return None,
            }
        }
        _ => return None,
    })
}

/// 툴팁 라벨 — 한글(현재 창 문안과 동일 언어 · i18n 스윙은 T-23).
fn tool_label(t: Tool) -> &'static str {
    match t {
        Tool::Pin => "고정/해제 (Ctrl+P)",
        Tool::Delete => "삭제 (Delete)",
        Tool::Copy => "복사 (Enter)",
        Tool::CopyPlain => "평문으로 복사 (Shift+Enter)", // ⇧는 맑은 고딕에 없다(두부 · 09-01)
        Tool::Settings => "설정",
    }
}

/// `Rect`에 (x,y) 포함 판정이 없어 로컬 확장.
trait ContainsXy {
    fn contains_xy(&self, x: i32, y: i32) -> bool;
}
impl ContainsXy for Rect {
    fn contains_xy(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}
