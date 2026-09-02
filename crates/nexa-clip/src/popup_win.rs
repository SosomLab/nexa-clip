//! S1 퀵 팝업 1단(T-18) — ★ **전역 단축키 → 커서 위치 팝업 → 고르면 붙는다**.
//!
//! 제품의 존재 이유가 걸린 왕복(K-1)이 처음으로 실데이터와 이어지는 자리다:
//! 단축키(트레이 스레드 `RegisterHotKey`) → **포커스 기억**([`nclip_plat::paste`]) →
//! 팝업(장식 없는 최상위 창 · 커서 위치 — DR-24) → ↑↓/타이핑 검색 → Enter →
//! **재적재 + 포커스 복원 + `Ctrl+V` 주입**.
//!
//! | 키 | 동작 |
//! |---|---|
//! | 타이핑 | 검색(라벨 부분 일치 · 대소문자 무시) |
//! | `↑`/`↓` | 선택 이동(선택이 보이게 따라 내려간다) |
//! | `Enter` | ★ **원본** — 표현 전부 재적재(DR-35) |
//! | `⇧Enter` | ★ **평문** — 평문 표현만(없으면 정직하게 거절) |
//! | `Esc` · 포커스 잃음 | 닫기(아무것도 안 붙인다) |
//!
//! 목록 렌더는 [`demo`](crate::demo)의 S1 화법(간략 보기)을 실데이터로 옮긴 것이다.
//! 가변 높이 가상화·미리보기 패널은 T-18 본편에서.

use nclip_core::history::History;
use nclip_core::{current_lang, tr, ClipKind, Msg, PasteAs};
use nclip_ctl::controls::{Control as _, ScrollBars, TextBox};
use nclip_ctl::draw::{DrawCtx, FontSlot};
use nclip_ctl::event::{InputEvent as CtlEvent, Key as CtlKey};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
use nclip_ctl::widget::{Invalidations, Widget as _};
use nclip_gfx::{Font, Surface};

use std::num::NonZeroU32;
use std::rc::Rc;

use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId, WindowLevel};

/// 팝업 논리 크기 — 목록 ~9행.
const POPUP_W: f64 = 380.0;
const POPUP_H: f64 = 400.0;

/// 팝업에서 셸로 되돌리는 행동.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PopupAction {
    /// 아무 일 없음(내부 처리 완료).
    None,
    /// 닫아 달라(Esc·포커스 잃음·X) — 붙여넣기 없음.
    Close,
    /// ★ 항목 선택 — `index`는 **이력 인덱스**(필터를 통과해 되돌린 값).
    Pick {
        /// 이력 인덱스(0 = 최신).
        index: usize,
        /// ★ 붙여넣기 방식(T-15b · 09-01 확정: ⇧=평문 · Ctrl=개체 · Alt=경로).
        as_: PasteAs,
    },
}

/// 목록 한 행(그리기용 사본) — 이력을 빌리지 않고 스냅숏으로 들어 수명을 끊는다.
struct Row {
    hist_index: usize,
    kind: ClipKind,
    label: String,
    copies: u32,
    /// 이미지 썸네일(설정이 켜져 있고 디코드가 됐을 때) — 없으면 글리프.
    thumb: Option<nclip_ctl::theme::IconImage>,
}

pub(crate) struct Popup {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    scale: f32,
    /// ★ 검색 입력(09-02 — "메인 검색창의 모든 기능을 팝업에도") — 이식 `TextBox`
    ///   정식 편집기: 캐럿·드래그 선택·×지우기·우클릭 편집 메뉴·IME preedit.
    search: TextBox,
    /// 캐럿 깜박임 위상(셸이 500ms마다 돌린다).
    caret_phase: bool,
    /// 필터 통과 행(최신이 위).
    rows: Vec<Row>,
    /// 선택(rows 인덱스).
    sel: usize,
    /// ★ 목록 세로 스크롤(**픽셀** · 09-02 실기 — 메인과 동일 규약).
    scroll: i32,
    /// 휠 소수 델타 누적기(터치패드).
    wheel_frac: f32,
    /// ★ 목록 오버레이 스크롤바(자동 숨김 · 09-02).
    bars: ScrollBars,
    shift: bool,
    ctrl: bool,
    alt: bool,
    /// ★ 한 번이라도 포커스를 받았는가 — **생성 직후의 `Focused(false)`로 닫히지 않게**.
    ///   (잠금 화면·창 관리자에 따라 초기 이벤트 순서가 다르다 — 08-28 실기.)
    was_focused: bool,
    /// 연 시각 — ★ **단축키의 `v`가 검색창으로 새는 것**을 막는 유예 기준(08-28 실기).
    opened_at: std::time::Instant,
    /// 마지막 커서 위치(winit은 클릭 이벤트에 좌표를 싣지 않는다).
    cursor: (i32, i32),
}

/// 열림 직후 문자 입력 유예 — 단축키(Ctrl+Shift+V)를 누른 손이 떨어지기 전의
/// 오토리피트·해제 순서 차이로 `v`가 새 창에 배달된다. 사람이 검색을 시작하는
/// 속도보다 짧고, 키 해제보다 길게.
const TYPE_GRACE: std::time::Duration = std::time::Duration::from_millis(150);

/// 좌상단 (x, y)를 **커서가 든 모니터** 안으로 되민다 — 팝업 전체가 화면에 보이게.
///
/// 물리 좌표 기준. 팝업 크기는 논리 상수 × 그 모니터의 배율(창 생성 전이라 창 배율을
/// 모른다 — 모니터 배율이 곧 그 값이다). 커서를 품은 모니터가 없으면(순간적 좌표 이상)
/// 주 모니터, 그것도 없으면 그대로 둔다(fail-soft — 안 뜨는 것보다 낫다).
fn clamp_to_monitor(el: &ActiveEventLoop, x: i32, y: i32) -> (i32, i32) {
    let contains = |m: &winit::monitor::MonitorHandle| {
        let p = m.position();
        let s = m.size();
        x >= p.x
            && y >= p.y
            && x < p.x + i32::try_from(s.width).unwrap_or(i32::MAX)
            && y < p.y + i32::try_from(s.height).unwrap_or(i32::MAX)
    };
    let Some(mon) = el
        .available_monitors()
        .find(contains)
        .or_else(|| el.primary_monitor())
    else {
        return (x, y);
    };
    let scale = mon.scale_factor();
    let pw = (POPUP_W * scale).ceil() as i32;
    let ph = (POPUP_H * scale).ceil() as i32;
    // ★ 작업 영역 우선(09-01 사용자 실기 "작업표시줄에 가린다") — Windows는 rcWork,
    //   없으면 모니터 전체로 폴백. 가장자리 5px(논리) 여유도 사용자 요청.
    let (ax, ay, aw, ah) = nclip_plat::screen::work_area_at(x, y).unwrap_or_else(|| {
        let (mp, ms) = (mon.position(), mon.size());
        (
            mp.x,
            mp.y,
            i32::try_from(ms.width).unwrap_or(i32::MAX),
            i32::try_from(ms.height).unwrap_or(i32::MAX),
        )
    });
    let margin = (5.0 * scale).round() as i32;
    let max_x = ax + aw - pw - margin;
    let max_y = ay + ah - ph - margin;
    // 작업 영역보다 팝업이 크면(극단) 좌상단 고정이 최선이다.
    (x.min(max_x).max(ax + margin), y.min(max_y).max(ay + margin))
}

/// 종류 배지 글리프 — ⚠️ 전부 KS X 1001(맑은 고딕 커버) — 이모지는 두부가 된다(08-27).
pub(crate) fn kind_glyph(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Text => "▤",
        ClipKind::RichText => "▧",
        ClipKind::Image => "▣",
        ClipKind::Files => "▦",
        ClipKind::Color => "◆",
        ClipKind::Object => "◇",
    }
}

impl Popup {
    pub(crate) fn new(font: Font) -> Self {
        Self {
            window: None,
            ctx: None,
            surface: None,
            font,
            theme: Theme::dark(),
            scale: 1.0,
            search: {
                let mut t = TextBox::new(tr(current_lang(), Msg::SearchHint)).with_clearable();
                t.set_focused(true);
                t
            },
            caret_phase: true,
            rows: Vec::new(),
            sel: 0,
            scroll: 0,
            wheel_frac: 0.0,
            bars: ScrollBars::new(),
            shift: false,
            ctrl: false,
            alt: false,
            was_focused: false,
            opened_at: std::time::Instant::now(),
            cursor: (0, 0),
        }
    }

    /// 좌표 → 목록 행(rows 인덱스) — 그리기와 같은 자(scale 기반)를 쓴다.
    fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        let Some(win) = &self.window else { return None };
        let size = win.inner_size();
        let px = |v: f32| (v * self.scale).round() as i32;
        let (header_h, footer_h, row_h) = (px(38.0), px(24.0), px(30.0).max(1));
        let list_bot = size.height as i32 - footer_h;
        if x < 0 || x >= size.width as i32 || y < header_h || y >= list_bot {
            return None;
        }
        let vi = ((y - header_h + self.scroll.max(0)) / row_h) as usize;
        (vi < self.rows.len()).then_some(vi)
    }

    /// 행 높이(px) — 레이아웃 공용 상수 30pt.
    fn row_h(&self) -> i32 {
        ((self.scale * 30.0).round() as i32).max(1)
    }

    /// 목록 사각형(검색바 아래 · 힌트 줄 위).
    fn list_vp(&self) -> Option<nclip_ctl::geom::Rect> {
        let win = self.window.as_ref()?;
        let sz = win.inner_size();
        let px = |v: f32| (v * self.scale).round() as i32;
        Some(nclip_ctl::geom::Rect::new(
            0,
            px(38.0),
            sz.width as i32,
            (sz.height as i32 - px(24.0) - px(38.0)).max(1),
        ))
    }

    /// ★ 스크롤바 우선 라우팅(썸 드래그) — 소비되면 true(09-02).
    fn feed_bars(&mut self, event: &WindowEvent) -> bool {
        let Some(vp) = self.list_vp() else {
            return false;
        };
        let Some(ev) = crate::main_win::to_ctl_event(event, self.cursor) else {
            return false;
        };
        if !matches!(
            ev,
            CtlEvent::MouseDown { .. } | CtlEvent::MouseMove { .. } | CtlEvent::MouseUp { .. }
        ) {
            return false;
        }
        #[allow(clippy::cast_possible_wrap)]
        let total = self.rows.len() as i32 * self.row_h();
        let (_, oy, consumed) =
            self.bars
                .on_event(&ev, vp, vp.w, total, 0, self.scroll, self.scale);
        if oy != self.scroll {
            self.scroll = oy;
            self.redraw();
        } else if consumed {
            self.redraw();
        }
        consumed
    }

    /// 스크롤바 페이드 틱(셸 500ms).
    pub(crate) fn tick_ui(&mut self, now_ms: u64) {
        if self.bars.tick(now_ms) {
            self.redraw();
        }
    }

    /// 캐럿 깜빡임 위상(셸 타이머) — 바뀌면 다시 그린다.
    /// 다시 그리기(셸 공개 — 언어 등 전역 변경 반영).
    pub(crate) fn redraw_public(&self) {
        self.redraw();
    }

    pub(crate) fn set_caret_phase(&mut self, on: bool) {
        if self.caret_phase != on {
            self.caret_phase = on;
            self.redraw();
        }
    }

    /// 검색 입력에 ctl 이벤트를 넣고, 편집 메뉴 액션·질의 변화를 수확한다.
    /// 질의가 바뀌었으면 true(호출측이 refresh).
    fn feed_search(&mut self, ev: &CtlEvent) -> bool {
        let before = self.search.display_text();
        let mut inv = Invalidations::default();
        self.search.on_event(ev, &mut inv);
        if let Some(act) = self.search.take_edit_ctx() {
            use nclip_ctl::controls::EditCtxAction as A;
            match act {
                A::Copy => {
                    if let Some(t) = self.search.copy_selection() {
                        crate::cliptext::set_text(&t);
                    }
                }
                A::Cut => {
                    if let Some(t) = self.search.cut_selection(&mut inv) {
                        crate::cliptext::set_text(&t);
                    }
                }
                A::Paste => {
                    if let Some(t) = crate::cliptext::get_text() {
                        self.search.paste(t.trim_end_matches('\n'), &mut inv);
                    }
                }
            }
        }
        if !inv.is_empty() {
            self.redraw();
        }
        self.search.display_text() != before
    }

    /// 눌린 수식 키 → 붙여넣기 모드(09-01 확정: ⇧ 평문 · Ctrl 개체 · Alt 경로 · 기본 원본).
    fn paste_mode(&self) -> PasteAs {
        if self.shift {
            PasteAs::Plain
        } else if self.ctrl {
            PasteAs::Object
        } else if self.alt {
            PasteAs::PathOnly
        } else {
            PasteAs::Original
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.window.is_some()
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    /// 커서 위치(주면)에서 연다 — 검색·선택은 초기화.
    /// 테마 교체(설정 창과 같은 실효값 — `ui.theme` · OS 선호).
    pub(crate) fn set_theme(&mut self, t: Theme) {
        if t.is_dark != self.theme.is_dark {
            self.theme = t;
            self.redraw();
        }
    }

    pub(crate) fn open(&mut self, el: &ActiveEventLoop, at: Option<(i32, i32)>, hist: &History) {
        if self.window.is_some() {
            return;
        }
        self.search.set_text("");
        self.search.set_focused(true);
        self.sel = 0;
        self.scroll = 0;
        self.was_focused = false;
        self.opened_at = std::time::Instant::now();
        self.refresh(hist);
        let mut attrs = crate::settings_win::win_name(crate::icon::with_icon(
            Window::default_attributes()
                .with_title("Nexa Clip")
                .with_decorations(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_inner_size(LogicalSize::new(POPUP_W, POPUP_H)),
        ));
        if let Some((x, y)) = at {
            // 커서 위치(DR-24 기본) — 물리 좌표 그대로(커서가 곧 물리 좌표다).
            // ★ 화면 경계 클램프(08-31 사용자 실기 "우측 하단이면 팝업이 잘린다") —
            //   커서가 든 모니터 안에 **전체가 들어가도록** 좌상단을 되민다.
            let (x, y) = clamp_to_monitor(el, x, y);
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        let Ok(win) = el.create_window(attrs) else {
            eprintln!("팝업 창 생성 실패");
            return;
        };
        let win = Rc::new(win);
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
        win.set_ime_allowed(true); // 한글 조합 이벤트(Ime::Preedit)를 받는다
        win.focus_window();
        self.window = Some(win);
    }

    pub(crate) fn close(&mut self) {
        self.surface = None;
        self.ctx = None;
        self.window = None;
    }

    /// 이력 → 필터 통과 행 재구성(검색어 변경·이력 변경 시).
    pub(crate) fn refresh(&mut self, hist: &History) {
        let q = self.search.display_text().to_lowercase();
        self.rows.clear();
        let mut i = 0usize;
        while let Some(item) = hist.get(i) {
            if q.is_empty() || item.label.to_lowercase().contains(&q) {
                self.rows.push(Row {
                    hist_index: i,
                    kind: item.kind,
                    label: item.label.clone(),
                    copies: item.copies,
                    thumb: item.thumb.as_ref().map(|(w, h, rgba)| {
                        nclip_ctl::theme::IconImage::from_rgba(*w, *h, rgba.clone())
                    }),
                });
            }
            i += 1;
        }
        if self.sel >= self.rows.len() {
            self.sel = self.rows.len().saturating_sub(1);
        }
        #[allow(clippy::cast_possible_wrap)]
        {
            self.scroll = self.scroll.min(self.sel as i32 * self.row_h());
        }
    }

    /// ★ 이력이 바뀌었다(새 복사·승격) — **커서를 맨 위로**(방금 것이 첫 줄이고
    /// 선택도 그걸 가리킨다 — 08-28 사용자 요청) + 다시 그린다.
    pub(crate) fn on_history_changed(&mut self, hist: &History) {
        self.sel = 0;
        self.scroll = 0;
        self.refresh(hist);
        self.redraw();
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// 창 이벤트 처리 — 셸이 [`PopupAction`]에 따라 후속(닫기·재적재)을 한다.
    pub(crate) fn handle_event(&mut self, event: &WindowEvent, hist: &History) -> PopupAction {
        match event {
            WindowEvent::CloseRequested => return PopupAction::Close,
            WindowEvent::Focused(true) => self.was_focused = true,
            // ★ 포커스 상실 = 닫기(Maccy 관례) — 단, **받아 본 적이 있을 때만**.
            //   생성 직후 `Focused(false)`가 먼저 오는 환경(잠금·일부 WM)에서
            //   열리자마자 닫히는 것을 막는다(08-28 실기).
            WindowEvent::Focused(false) if self.was_focused => return PopupAction::Close,
            WindowEvent::Focused(false) => {}
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = *scale_factor as f32;
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.ctrl = m.state().control_key();
                self.alt = m.state().alt_key();
            }
            // ★ 한글 조합 중 검색(T-17/T-18 · FR-F-2) — Preedit가 올 때마다 실시간 필터,
            //   Commit은 버퍼에 확정. (winit `set_ime_allowed(true)` — open에서 켜 둔다.)
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
                    self.sel = 0;
                    self.scroll = 0;
                    self.refresh(hist);
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if self.feed_bars(event) {
                    return PopupAction::None;
                }
                let (x, y) = self.cursor;
                if self.feed_search(&CtlEvent::MouseMove { x, y }) {
                    self.sel = 0;
                    self.scroll = 0;
                    self.refresh(hist);
                }
            }
            // ★ 클릭 = 선택 + 붙여넣기(Maccy 관례 — 08-28 사용자 실기 "클릭 선택 안 됨").
            //   `⇧` 클릭 = 평문. 목록 밖 클릭은 무시(닫기는 Esc·바깥 포커스가 담당).
            WindowEvent::MouseInput { state, button, .. } => {
                if self.feed_bars(event) {
                    return PopupAction::None;
                }
                let (x, y) = self.cursor;
                // 우클릭 = 검색창 편집 메뉴(09-02) — 메뉴가 열려 있는 동안은 전부 검색 몫.
                if *state == ElementState::Pressed
                    && *button == winit::event::MouseButton::Right
                    && self
                        .search
                        .bounds()
                        .contains(nclip_ctl::geom::Point { x, y })
                {
                    self.search
                        .set_clipboard_has_text(crate::cliptext::has_text());
                    let _ = self.feed_search(&CtlEvent::RightDown { x, y });
                    return PopupAction::None;
                }
                if self.search.popup_open() {
                    let ev = match (state, button) {
                        (ElementState::Pressed, winit::event::MouseButton::Left) => {
                            Some(CtlEvent::MouseDown {
                                x,
                                y,
                                shift: self.shift,
                                primary: self.ctrl,
                            })
                        }
                        (ElementState::Released, winit::event::MouseButton::Left) => {
                            Some(CtlEvent::MouseUp { x, y })
                        }
                        _ => None,
                    };
                    if let Some(ev) = ev {
                        if self.feed_search(&ev) {
                            self.sel = 0;
                            self.scroll = 0;
                            self.refresh(hist);
                        }
                    }
                    return PopupAction::None;
                }
                if *state == ElementState::Released && *button == winit::event::MouseButton::Left {
                    let _ = self.feed_search(&CtlEvent::MouseUp { x, y });
                }
                if *state == ElementState::Pressed && *button == winit::event::MouseButton::Left {
                    if self
                        .search
                        .bounds()
                        .contains(nclip_ctl::geom::Point { x, y })
                    {
                        if self.feed_search(&CtlEvent::MouseDown {
                            x,
                            y,
                            shift: self.shift,
                            primary: self.ctrl,
                        }) {
                            self.sel = 0;
                            self.scroll = 0;
                            self.refresh(hist);
                        }
                        self.redraw();
                        return PopupAction::None;
                    }
                    if let Some(vi) = self.row_at(x, y) {
                        self.sel = vi;
                        self.redraw();
                        if let Some(row) = self.rows.get(vi) {
                            return PopupAction::Pick {
                                index: row.hist_index,
                                as_: self.paste_mode(),
                            };
                        }
                    }
                }
            }
            // ★ 휠 = 픽셀 스크롤(09-02 실기 — 행 단위·정수 절단이 패드에서 뚝뚝 끊겼다).
            //   선택은 그대로 두고 화면만 움직인다.
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        -y * (self.row_h() * 3) as f32
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                };
                self.wheel_frac += dy;
                #[allow(clippy::cast_possible_truncation)]
                let d = self.wheel_frac as i32;
                if d != 0 {
                    self.wheel_frac -= d as f32;
                    self.scroll += d; // 상한은 paint에서 죈다.
                    self.bars.show();
                    self.redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return PopupAction::None;
                }
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => return PopupAction::Close,
                    Key::Named(NamedKey::ArrowUp) => {
                        self.sel = self.sel.saturating_sub(1);
                        #[allow(clippy::cast_possible_wrap)]
                        {
                            self.scroll = self.scroll.min(self.sel as i32 * self.row_h());
                        }
                        self.redraw();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.sel + 1 < self.rows.len() {
                            self.sel += 1;
                        }
                        self.redraw();
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some(row) = self.rows.get(self.sel) {
                            return PopupAction::Pick {
                                index: row.hist_index,
                                as_: self.paste_mode(),
                            };
                        }
                        return PopupAction::Close;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if self.feed_search(&CtlEvent::Char {
                            c: '\u{8}',
                            now_ms: 0,
                        }) {
                            self.sel = 0;
                            self.scroll = 0;
                            self.refresh(hist);
                        }
                        self.redraw();
                    }
                    // ★ 캐럿 이동·범위 선택(⇧) — 메인 검색과 동일(09-02).
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
                        let _ = self.feed_search(&CtlEvent::Key {
                            key,
                            shift: self.shift,
                            primary: self.ctrl,
                        });
                        self.redraw();
                    }
                    // ★ 클립보드 단축(09-02 — 메인과 동일). Ctrl+V는 유예와 무관.
                    Key::Character("a" | "A") if self.ctrl => {
                        let _ = self.feed_search(&CtlEvent::SelectAll);
                        self.redraw();
                    }
                    Key::Character("c" | "C") if self.ctrl => {
                        if let Some(t) = self.search.copy_selection() {
                            crate::cliptext::set_text(&t);
                        }
                    }
                    Key::Character("x" | "X") if self.ctrl => {
                        let mut inv = Invalidations::default();
                        if let Some(t) = self.search.cut_selection(&mut inv) {
                            crate::cliptext::set_text(&t);
                            self.sel = 0;
                            self.scroll = 0;
                            self.refresh(hist);
                            self.redraw();
                        }
                    }
                    Key::Character("v" | "V")
                        if self.ctrl && self.opened_at.elapsed() >= TYPE_GRACE =>
                    {
                        if let Some(t) = crate::cliptext::get_text() {
                            let mut inv = Invalidations::default();
                            self.search.paste(t.trim_end_matches('\n'), &mut inv);
                            self.sel = 0;
                            self.scroll = 0;
                            self.refresh(hist);
                            self.redraw();
                        }
                    }
                    _ => {
                        // ★ 단축키 잔향 차단(08-28 실기) — 열림 직후 유예 동안과
                        //   Ctrl이 눌린 동안의 문자는 검색어가 아니다(`v` 유출).
                        if self.ctrl || self.opened_at.elapsed() < TYPE_GRACE {
                            return PopupAction::None;
                        }
                        if let Some(txt) = event.text.as_ref() {
                            let mut changed = false;
                            for c in txt.chars().filter(|c| !c.is_control()) {
                                if self.feed_search(&CtlEvent::Char { c, now_ms: 0 }) {
                                    changed = true;
                                }
                            }
                            if changed {
                                self.sel = 0;
                                self.scroll = 0;
                                self.refresh(hist);
                                self.redraw();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        PopupAction::None
    }

    fn paint(&mut self) {
        // 검색창 배치 — 그리기 전에(빌림 분리).
        if let Some(win) = &self.window {
            let size = win.inner_size();
            let px = |v: f32| (v * self.scale).round() as i32;
            let mut inv = Invalidations::default();
            self.search.set_scale(self.scale);
            self.search.set_bounds(
                Rect::new(
                    px(10.0),
                    px(7.0),
                    (size.width as i32 - px(20.0)).max(40),
                    px(24.0),
                ),
                &mut inv,
            );
        }
        let (Some(win), Some(surface)) = (self.window.clone(), self.surface.as_mut()) else {
            return;
        };
        let size = win.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        // ★ 선택 가시화 스크롤은 **그리기 전에** 계산한다 — 그리는 동안 self를
        //   다시 빌리지 않기 위해(필드 분리 빌림).
        let (iw, ih) = (size.width as i32, size.height as i32);
        let px = |v: f32| (v * self.scale).round() as i32;
        let row_h = px(30.0).max(1);
        let list_h = ((ih - px(24.0)) - px(38.0)).max(1);
        #[allow(clippy::cast_possible_wrap)]
        {
            let max_scroll = (self.rows.len() as i32 * row_h - list_h).max(0);
            self.scroll = self.scroll.clamp(0, max_scroll);
            let sel_bot = (self.sel as i32 + 1) * row_h;
            if sel_bot > self.scroll + list_h {
                self.scroll = sel_bot - list_h;
            }
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        {
            let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
            // ★ 배율은 레이아웃과 같은 값(08-27 macOS 회귀의 교훈).
            let mut dc =
                RasterCtx::new(&mut gfx, &self.font, self.scale).with_caret_on(self.caret_phase);
            draw(
                &mut dc,
                iw,
                ih,
                self.scale,
                self.theme,
                &self.search,
                &self.rows,
                self.sel,
                self.scroll,
            );
            #[allow(clippy::cast_possible_wrap)]
            let total = self.rows.len() as i32 * row_h;
            let vp = nclip_ctl::geom::Rect::new(0, px(38.0), iw, (ih - px(24.0) - px(38.0)).max(1));
            self.bars.paint(
                &mut dc,
                &self.theme,
                vp,
                vp.w,
                total,
                0,
                self.scroll,
                self.scale,
            );
        }
        let _ = buf.present();
    }
}

/// 팝업 화면 — [`demo`](crate::demo)의 간략 보기 화법을 실데이터로.
#[allow(clippy::too_many_arguments)]
fn draw(
    dc: &mut RasterCtx<'_, '_, '_>,
    w: i32,
    h: i32,
    s: f32,
    th: Theme,
    search: &TextBox,
    rows: &[Row],
    sel: usize,
    scroll: i32,
) {
    let px = |v: f32| (v * s).round() as i32;
    let full = Rect::new(0, 0, w, h);
    dc.select_font(FontSlot::Base, false);

    let header_h = px(38.0);
    let footer_h = px(24.0);
    let pad = px(10.0);
    let row_h = px(30.0).max(1);

    dc.fill_rect(full, th.window_bg);
    // 팝업 테두리 — 장식 없는 창이라 우리가 그린다.
    dc.fill_rect(Rect::new(0, 0, w, 1), th.border);
    dc.fill_rect(Rect::new(0, h - 1, w, 1), th.border);
    dc.fill_rect(Rect::new(0, 0, 1, h), th.border);
    dc.fill_rect(Rect::new(w - 1, 0, 1, h), th.border);

    // ── 헤더: 검색 필드(정식 TextBox — 캐럿·선택·×· 09-02) ──
    dc.fill_rect(Rect::new(0, 0, w, header_h), th.chrome_bg);
    search.paint(dc, &th);
    dc.fill_rect(Rect::new(0, header_h - 1, w, 1), th.border);

    // ── 목록(간략 보기 화법) ──
    let list_top = header_h;
    let list_bot = h - footer_h;
    let start = (scroll / row_h).max(0) as usize;
    let off = scroll.rem_euclid(row_h);
    let visible = ((list_bot - list_top + off + row_h - 1) / row_h).max(1) as usize;
    if rows.is_empty() {
        let lang = current_lang();
        let msg = if search.display_text().is_empty() {
            tr(lang, Msg::PopupNoItems)
        } else {
            tr(lang, Msg::MainNoMatch)
        };
        dc.text(pad, list_top + px(12.0), full, msg, th.text_dim);
    }
    for (vi, row) in rows.iter().enumerate().skip(start).take(visible) {
        let y = list_top - off + ((vi - start) as i32) * row_h;
        let cy0 = y.max(list_top);
        let clip = Rect::new(0, cy0, w, ((y + row_h).min(list_bot) - cy0).max(0));
        if vi == sel {
            dc.fill_rect(clip, th.sel_bg);
        } else if vi % 2 == 1 {
            dc.fill_rect(clip, th.panel_bg_alt);
        }
        // ★ 이미지 썸네일(설정 켜짐 · 디코드 성공 시) — 비율 유지로 24px 상자에.
        if let Some(img) = &row.thumb {
            let box_side = px(24.0);
            let (iw, ih) = (img.w.max(1) as i32, img.h.max(1) as i32);
            let (dw, dh) = if iw >= ih {
                (box_side, (box_side * ih / iw).max(1))
            } else {
                ((box_side * iw / ih).max(1), box_side)
            };
            let dst = Rect::new(pad + (box_side - dw) / 2, y + (row_h - dh) / 2, dw, dh);
            dc.image_scaled(dst, img, clip);
        } else {
            dc.text(pad, y + px(7.0), clip, kind_glyph(row.kind), th.accent);
        }
        dc.text(pad + px(30.0), y + px(7.0), clip, &row.label, th.text);
        if row.copies > 1 {
            let tag = format!("×{}", row.copies);
            let tw = dc.text_width(&tag);
            dc.text(w - pad - tw, y + px(7.0), clip, &tag, th.text_dim);
        }
    }

    // ── 푸터: 키 힌트 1줄 ──
    let fy = h - footer_h;
    dc.fill_rect(Rect::new(0, fy, w, footer_h), th.chrome_bg);
    dc.fill_rect(Rect::new(0, fy, w, 1), th.border);
    dc.text(
        pad,
        fy + px(5.0),
        full,
        // ★ 힌트는 선택 항목 종류를 따른다(DR-35 · 09-01 키 배치 확정).
        {
            let lang = current_lang();
            match rows.get(sel).map(|r| r.kind) {
                Some(ClipKind::Files) => tr(lang, Msg::HintFiles),
                Some(ClipKind::RichText) => tr(lang, Msg::HintRich),
                Some(ClipKind::Image | ClipKind::Object) => tr(lang, Msg::HintImage),
                _ => tr(lang, Msg::HintDefault),
            }
        },
        th.text_dim,
    );

    // 검색 우클릭 편집 메뉴 — 맨 위 레이어(z = 그리는 순서).
    search.paint_popup(dc, &th);
}
