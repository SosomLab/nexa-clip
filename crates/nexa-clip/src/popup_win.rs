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
use nclip_core::ClipKind;
use nclip_ctl::draw::{DrawCtx, FontSlot};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
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
        /// `⇧Enter` = 평문만.
        plain: bool,
    },
}

/// 목록 한 행(그리기용 사본) — 이력을 빌리지 않고 스냅숏으로 들어 수명을 끊는다.
struct Row {
    hist_index: usize,
    kind: ClipKind,
    label: String,
    copies: u32,
}

pub(crate) struct Popup {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    scale: f32,
    query: String,
    /// 필터 통과 행(최신이 위).
    rows: Vec<Row>,
    /// 선택(rows 인덱스).
    sel: usize,
    /// 스크롤 시작 행(선택을 따라간다).
    top: usize,
    shift: bool,
    /// ★ 한 번이라도 포커스를 받았는가 — **생성 직후의 `Focused(false)`로 닫히지 않게**.
    ///   (잠금 화면·창 관리자에 따라 초기 이벤트 순서가 다르다 — 08-28 실기.)
    was_focused: bool,
}

/// 종류 배지 글리프 — ⚠️ 전부 KS X 1001(맑은 고딕 커버) — 이모지는 두부가 된다(08-27).
fn kind_glyph(kind: ClipKind) -> &'static str {
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
            query: String::new(),
            rows: Vec::new(),
            sel: 0,
            top: 0,
            shift: false,
            was_focused: false,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.window.is_some()
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    /// 커서 위치(주면)에서 연다 — 검색·선택은 초기화.
    pub(crate) fn open(&mut self, el: &ActiveEventLoop, at: Option<(i32, i32)>, hist: &History) {
        if self.window.is_some() {
            return;
        }
        self.query.clear();
        self.sel = 0;
        self.top = 0;
        self.was_focused = false;
        self.refresh(hist);
        let mut attrs = Window::default_attributes()
            .with_title("Nexa Clip")
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(POPUP_W, POPUP_H));
        if let Some((x, y)) = at {
            // 커서 위치(DR-24 기본) — 물리 좌표 그대로(커서가 곧 물리 좌표다).
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
        let q = self.query.to_lowercase();
        self.rows.clear();
        let mut i = 0usize;
        while let Some(item) = hist.get(i) {
            if q.is_empty() || item.label.to_lowercase().contains(&q) {
                self.rows.push(Row {
                    hist_index: i,
                    kind: item.kind,
                    label: item.label.clone(),
                    copies: item.copies,
                });
            }
            i += 1;
        }
        if self.sel >= self.rows.len() {
            self.sel = self.rows.len().saturating_sub(1);
        }
        self.top = self.top.min(self.sel);
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
            WindowEvent::ModifiersChanged(m) => self.shift = m.state().shift_key(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return PopupAction::None;
                }
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => return PopupAction::Close,
                    Key::Named(NamedKey::ArrowUp) => {
                        self.sel = self.sel.saturating_sub(1);
                        self.top = self.top.min(self.sel);
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
                                plain: self.shift,
                            };
                        }
                        return PopupAction::Close;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.query.pop();
                        self.sel = 0;
                        self.top = 0;
                        self.refresh(hist);
                        self.redraw();
                    }
                    _ => {
                        if let Some(txt) = event.text.as_ref() {
                            let mut changed = false;
                            for c in txt.chars().filter(|c| !c.is_control()) {
                                self.query.push(c);
                                changed = true;
                            }
                            if changed {
                                self.sel = 0;
                                self.top = 0;
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
        let visible = (((ih - px(24.0)) - px(38.0)) / row_h).max(1) as usize;
        if self.sel >= self.top + visible {
            self.top = self.sel + 1 - visible;
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        {
            let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
            // ★ 배율은 레이아웃과 같은 값(08-27 macOS 회귀의 교훈).
            let mut dc = RasterCtx::new(&mut gfx, &self.font, self.scale);
            draw(
                &mut dc,
                iw,
                ih,
                self.scale,
                self.theme,
                &self.query,
                &self.rows,
                self.sel,
                self.top,
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
    query: &str,
    rows: &[Row],
    sel: usize,
    top: usize,
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

    // ── 헤더: 검색 필드(항상 포커스) ──
    dc.fill_rect(Rect::new(0, 0, w, header_h), th.chrome_bg);
    dc.fill_round_rect(
        Rect::new(pad, px(7.0), w - pad * 2, px(24.0)),
        px(6.0),
        th.field_bg,
    );
    if query.is_empty() {
        dc.text(pad + px(8.0), px(11.0), full, "검색…", th.text_dim);
    } else {
        dc.text(pad + px(8.0), px(11.0), full, query, th.text);
    }
    dc.fill_rect(Rect::new(0, header_h - 1, w, 1), th.border);

    // ── 목록(간략 보기 화법) ──
    let list_top = header_h;
    let list_bot = h - footer_h;
    let visible = ((list_bot - list_top) / row_h).max(1) as usize;
    if rows.is_empty() {
        let msg = if query.is_empty() {
            "아직 잡은 항목이 없습니다 — 복사해 보세요"
        } else {
            "일치하는 항목이 없습니다"
        };
        dc.text(pad, list_top + px(12.0), full, msg, th.text_dim);
    }
    for (vi, row) in rows.iter().enumerate().skip(top).take(visible) {
        let y = list_top + ((vi - top) as i32) * row_h;
        let clip = Rect::new(0, y, w, row_h.min(list_bot - y));
        if vi == sel {
            dc.fill_rect(clip, th.sel_bg);
        } else if vi % 2 == 1 {
            dc.fill_rect(clip, th.panel_bg_alt);
        }
        dc.text(pad, y + px(7.0), clip, kind_glyph(row.kind), th.accent);
        dc.text(pad + px(20.0), y + px(7.0), clip, &row.label, th.text);
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
        "Enter 원본 · ⇧Enter 평문 · Esc 닫기",
        th.text_dim,
    );
}
