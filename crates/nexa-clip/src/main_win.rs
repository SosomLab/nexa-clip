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
use nclip_core::{current_lang, tr, ClipKind, Msg, PasteAs};
use nclip_ctl::controls::{
    ContextMenu, Control as _, CtxItem, LabelSide, ScrollBars, Switch, TextBox,
};
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
/// 메인창 열기 옵션(설정 스냅샷) — 인자 폭발 방지(clippy 8/7).
pub(crate) struct OpenOpts<'a> {
    /// `ui.view_mode` 코드.
    pub view_code: &'a str,
    /// `ui.always_on_top`.
    pub always_top: bool,
    /// `ui.preview_open`(09-02 K4).
    pub preview_open: bool,
}

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
    /// ★ 최상위 고정 토글(09-02) — 셸이 `ui.always_on_top` 영속 + 창 레벨 적용.
    ToggleAlwaysTop,
    /// ★ 미리보기 패널 토글(09-02 K4) — 셸이 `ui.preview_open` 영속.
    TogglePreview,
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
    /// 원본 화소 치수(라벨 "W×H" 파싱 · 09-02 가변 행 — 섬네일은 축소본이라 절대 크기를 모른다).
    img_dims: Option<(u32, u32)>,
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
    /// ★ 미리보기 패널 토글(09-02 K4 — CopyQ식 하단 패널). 켜짐 = accent.
    Preview,
    /// ★ 최상위 고정(09-02 사용자 요청) — 모든 창 위에 표시. ⚙ 위 바닥 고정.
    AlwaysTop,
    Settings,
}

/// 툴바 배치(위에서부터) — `None` = 구분선. ⚙는 바닥 고정(VT-4).
const TOOLS_TOP: [Option<Tool>; 7] = [
    Some(Tool::Pin),
    Some(Tool::Delete),
    None,
    Some(Tool::Copy),
    Some(Tool::CopyPlain),
    None,
    Some(Tool::Preview),
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
    /// 캐럿 깜빡임 위상(셸 500ms 타이머).
    caret_phase: bool,
    /// ★ 최상위 고정 상태(`ui.always_on_top` 영속 — 셸이 넘겨준다).
    always_top: bool,
    rows: Vec<Row>,
    sel: usize,
    /// ★ 목록 세로 스크롤(**픽셀** · 09-02 실기 — 행 단위는 터치패드에서 뚝뚝 끊겼다).
    scroll: i32,
    /// 휠 소수 델타 누적기 — 패드의 0.1노치도 쌓여서 움직인다(절단 0 방지).
    wheel_frac: f32,
    /// ★ 행 시작 y 누적합(len = rows+1 · 마지막 = 총 높이) — Rich 가변 행(09-02 CopyQ 화법).
    row_offs: Vec<i32>,
    /// ★ 목록 오버레이 스크롤바(설정창과 동일 부품 · 자동 숨김 — 09-02 사용자 요청).
    bars: ScrollBars,
    cursor: (i32, i32),
    shift: bool,
    primary: bool,
    alt: bool,
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
    /// ★ 편집 시트 우상단 줄 바꿈 스위치(09-02 M4 — Alt+Z와 동기).
    wrap_sw: Option<Switch>,
    /// 상태줄에 보일 전체 개수(필터 전).
    total: usize,
    /// ★ 미리보기 패널 열림(09-02 K4 · `ui.preview_open` 영속 — 기본 접힘).
    preview_open: bool,
    /// 미리보기 텍스트 — (항목 id, 읽기용 멀티라인 · wrap · 휠 스크롤만 라우팅).
    preview_tb: Option<(u64, TextBox)>,
    /// 미리보기 이미지 원본 — (항목 id, 셸이 지연 디코드해 넘긴 RGBA ·
    /// `None` = 디코드 실패 → 텍스트 폴백 · 09-02 실기 P).
    preview_img: Option<(u64, Option<nclip_ctl::theme::IconImage>)>,
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
                let mut t = TextBox::new(tr(current_lang(), Msg::SearchHint)).with_clearable();
                t.set_focused(true);
                t
            },
            caret_phase: true,
            always_top: false,
            rows: Vec::new(),
            sel: 0,
            scroll: 0,
            wheel_frac: 0.0,
            row_offs: Vec::new(),
            bars: ScrollBars::new(),
            cursor: (0, 0),
            shift: false,
            primary: false,
            alt: false,
            last_click: None,
            hovered: None,
            view: ViewMode::Compact,
            menu: ContextMenu::new(),
            editor: None,
            wrap_sw: None,
            total: 0,
            preview_open: false,
            preview_tb: None,
            preview_img: None,
        }
    }

    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    /// Alt가 눌린 채인가 — winit 수식 상태를 에디터 경로에서도 쓰기 위한 도우미.
    #[allow(clippy::unused_self)]
    fn alt_down(&self, _kev: &winit::event::KeyEvent) -> bool {
        self.alt
    }

    /// 검색 우클릭 편집 메뉴의 선택을 실행한다(복사/잘라내기/붙여넣기).
    fn drain_search_edit_ctx(&mut self) {
        if let Some(act) = self.search.take_edit_ctx() {
            use nclip_ctl::controls::EditCtxAction as A;
            let mut inv = Invalidations::default();
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
    }

    /// 캐럿 깜빡임 위상(셸 타이머) — 바뀌면 다시 그린다.
    pub(crate) fn set_caret_phase(&mut self, on: bool) {
        if self.caret_phase != on {
            self.caret_phase = on;
            self.redraw();
        }
    }

    /// ★ 최상위 고정 적용(09-02) — 토글 즉시 창 레벨 반영.
    pub(crate) fn apply_always_top(&mut self, on: bool) {
        self.always_top = on;
        if let Some(w) = &self.window {
            w.set_window_level(if on {
                winit::window::WindowLevel::AlwaysOnTop
            } else {
                winit::window::WindowLevel::Normal
            });
        }
        self.redraw();
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
        opts: OpenOpts<'_>,
    ) {
        self.theme = theme;
        self.always_top = opts.always_top;
        self.preview_open = opts.preview_open;
        self.view = ViewMode::from_code(opts.view_code).unwrap_or_default();
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
        self.scroll = 0;
        self.refresh(hist);
        let attrs = crate::settings_win::win_name(crate::icon::with_icon(
            Window::default_attributes()
                .with_title(if cfg!(target_os = "linux") {
                    "Nexa Clip".to_string()
                } else {
                    format!("Nexa Clip — {}", tr(current_lang(), Msg::MainTitleSuffix))
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
        if self.always_top {
            win.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }
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
                img_dims: display_dims(&item.reps).or_else(|| parse_dims(&item.label)),
                plain: plain_of(&item.reps).or_else(|| svg_text(&item.reps)),
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
        // 스크롤은 paint에서 최대값으로만 죄인다 — 휠 위치를 존중(09-02).
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
            (h - self.header_h() - self.status_h() - self.preview_h_of(h)).max(0),
        )
    }

    /// ★ 미리보기 패널 높이(09-02 K4) — 목록 영역의 35% · 최소 80px.
    fn preview_h_of(&self, h: i32) -> i32 {
        if !self.preview_open {
            return 0;
        }
        let avail = (h - self.header_h() - self.status_h()).max(0);
        (avail * 35 / 100).max(self.px(80.0)).min(avail)
    }

    /// 미리보기 패널 사각형 — 상태줄 위 · 툴바 오른쪽.
    fn preview_rect(&self, w: i32, h: i32) -> Rect {
        let ph = self.preview_h_of(h);
        Rect::new(
            self.toolbar_w(),
            h - self.status_h() - ph,
            (w - self.toolbar_w()).max(0),
            ph,
        )
    }

    fn tool_rect(&self, slot: usize) -> Rect {
        let side = self.px(28.0);
        let x = (self.toolbar_w() - side) / 2;
        let mut y = self.px(6.0); // ★ 전고 툴바(09-02) — 헤더와 무관하게 맨 위부터.
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

    /// ★ 최상위 고정 — ⚙ 바로 위(바닥 구역 · 09-02).
    fn always_top_rect(&self, h: i32) -> Rect {
        let side = self.px(28.0);
        Rect::new(
            (self.toolbar_w() - side) / 2,
            h - self.status_h() - side * 2 - self.px(12.0),
            side,
            side,
        )
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
        if self.always_top_rect(h).contains_xy(x, y) {
            return Some(Tool::AlwaysTop);
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

    /// Rich 이미지 표시 크기 — 원본 크기 그대로(확대 없음) · 폭·최대 높이에 맞춰
    /// **비율 축소**(09-02 사용자 — 복사본마다 확대율이 제각각이던 문제).
    fn rich_fit(&self, ow: i32, oh: i32, content_w: i32) -> (i32, i32) {
        let max_h = self.px(200.0);
        // ★ 기본 배율 64%(09-02 사용자 — 80%에서 "80% 수준으로 더" = 0.8×0.8) —
        //   목록은 훑기용이라 실물보다 작은 쪽이 보기 좋다.
        let (ow, oh) = ((ow * 16 / 25).max(1), (oh * 16 / 25).max(1));
        let mut dw = ow.max(1).min(content_w.max(40));
        let mut dh = (oh.max(1) * dw / ow.max(1)).max(1);
        if dh > max_h {
            dw = (dw * max_h / dh).max(1);
            dh = max_h;
        }
        (dw, dh)
    }

    /// 콘텐츠 폭(거터·여백 제외) — 높이 계산과 그리기가 같은 값을 쓴다.
    fn content_w_of(&self, list_w: i32) -> i32 {
        (list_w - self.px(10.0) * 2 - self.px(30.0)).max(40)
    }

    /// 행 높이 — Compact·Plain은 고정, ★ Rich는 내용 기반 가변(이미지 = 실치수 비율
    /// 최대 200px · 텍스트 = 본문 줄 수 ≤5).
    fn row_height_of(&self, row: &Row, content_w: i32) -> i32 {
        if self.view != ViewMode::Rich {
            return self.row_h();
        }
        if let Some(img) = &row.thumb {
            #[allow(clippy::cast_possible_wrap)]
            let (ow, oh) = row
                .img_dims
                .map_or((img.w.max(1) as i32, img.h.max(1) as i32), |(a, b)| {
                    (a as i32, b as i32)
                });
            let (_, dh) = self.rich_fit(ow, oh, content_w);
            dh + self.px(12.0)
        } else {
            let text = row.plain.as_deref().unwrap_or(row.label.as_str());
            #[allow(clippy::cast_possible_wrap)]
            let n = text.lines().take(5).count().max(1) as i32;
            self.px(12.0) + self.px(22.0) * n
        }
    }

    /// 누적합 재계산 — 매 paint(폭 의존) · O(n) 단순 산술.
    fn rebuild_offsets(&mut self, list_w: i32) {
        let cw = self.content_w_of(list_w);
        let mut offs = Vec::with_capacity(self.rows.len() + 1);
        offs.push(0i32);
        let mut acc = 0i32;
        for row in &self.rows {
            acc += self.row_height_of(row, cw);
            offs.push(acc);
        }
        self.row_offs = offs;
    }

    fn row_at(&self, x: i32, y: i32, w: i32, h: i32) -> Option<usize> {
        let l = self.list_rect(w, h);
        if x < l.x || x >= l.x + l.w || y < l.y || y >= l.y + l.h {
            return None;
        }
        let ly = y - l.y + self.scroll.max(0);
        if self.row_offs.len() < 2 || ly >= *self.row_offs.last().unwrap_or(&0) {
            return None;
        }
        let vi = self
            .row_offs
            .partition_point(|&o| o <= ly)
            .saturating_sub(1);
        (vi < self.rows.len()).then_some(vi)
    }

    fn selected_id(&self) -> Option<u64> {
        self.rows.get(self.sel).map(|r| r.id)
    }

    fn act(&self, tool: Tool) -> MainAction {
        match (tool, self.selected_id()) {
            (Tool::Settings, _) => MainAction::OpenSettings,
            (Tool::AlwaysTop, _) => MainAction::ToggleAlwaysTop,
            (Tool::Preview, _) => MainAction::TogglePreview,
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

    /// ★ 스크롤바에 마우스 사건을 먼저 준다(썸 드래그 · 09-02) — 소비되면 true.
    fn feed_bars(&mut self, event: &WindowEvent) -> bool {
        let Some(win) = &self.window else {
            return false;
        };
        let Some(ev) = to_ctl_event(event, self.cursor) else {
            return false;
        };
        if !matches!(
            ev,
            CtlEvent::MouseDown { .. } | CtlEvent::MouseMove { .. } | CtlEvent::MouseUp { .. }
        ) {
            return false;
        }
        let sz = win.inner_size();
        let list = self.list_rect(sz.width as i32, sz.height as i32);
        let total = *self.row_offs.last().unwrap_or(&0);
        let (_, oy, consumed) =
            self.bars
                .on_event(&ev, list, list.w, total, 0, self.scroll, self.scale);
        if oy != self.scroll {
            self.scroll = oy;
            self.redraw();
        } else if consumed {
            self.redraw();
        }
        consumed
    }

    /// 스크롤바 자동 숨김 페이드 틱(셸 500ms 심장 박동).
    pub(crate) fn tick_ui(&mut self, now_ms: u64) {
        if self.bars.tick(now_ms) {
            self.redraw();
        }
    }

    /// ★ 미리보기 내용을 현재 선택과 동기(09-02 K4) — id 비교만이라 매 사건 호출해도 값싸다.
    fn sync_preview(&mut self) {
        if !self.preview_open {
            self.preview_tb = None;
            self.preview_img = None;
            return;
        }
        let Some(row) = self.rows.get(self.sel) else {
            self.preview_tb = None;
            self.preview_img = None;
            return;
        };
        // ★ Object(PPT 글상자 등)도 이미지 표현이 있으면 그려 보인다(09-02 실기 P —
        //   SVG 마크업 원문이 텍스트로 떴다). 셸 디코드 실패만 텍스트로 폴백.
        if matches!(row.kind, ClipKind::Image | ClipKind::Object) {
            match &self.preview_img {
                // 디코드 성공 — 이미지가 그려진다.
                Some((id, Some(_))) if *id == row.id => {
                    self.preview_tb = None;
                    return;
                }
                // 디코드 실패 판정 — 아래 텍스트 폴백으로 흐른다.
                Some((id, None)) if *id == row.id => {}
                // 아직 디코드 대기(셸 펌프가 곧 채운다).
                _ => {
                    self.preview_tb = None;
                    return;
                }
            }
        } else {
            self.preview_img = None;
        }
        if self.preview_tb.as_ref().map(|(id, _)| *id) != Some(row.id) {
            let text = row.plain.clone().unwrap_or_else(|| row.label.clone());
            let mut tb = TextBox::new("").with_multiline().with_text(&text);
            tb.set_wrap(true);
            tb.set_scale(self.scale);
            self.preview_tb = Some((row.id, tb));
        }
    }

    /// ★ 셸에 묻는다 — 원본 이미지 디코드가 필요한 항목 id(미리보기 열림 + 이미지 선택 + 미보유).
    pub(crate) fn take_preview_request(&self) -> Option<u64> {
        if !self.preview_open || self.window.is_none() {
            return None;
        }
        let row = self.rows.get(self.sel)?;
        if !matches!(row.kind, ClipKind::Image | ClipKind::Object) {
            return None;
        }
        if self.preview_img.as_ref().map(|(id, _)| *id) == Some(row.id) {
            return None;
        }
        Some(row.id)
    }

    /// 셸이 디코드한 원본 이미지를 받는다.
    pub(crate) fn set_preview_image(&mut self, id: u64, iw: u32, ih: u32, rgba: Vec<u8>) {
        self.preview_img = Some((
            id,
            Some(nclip_ctl::theme::IconImage::from_rgba(iw, ih, rgba)),
        ));
        self.redraw();
    }

    /// 셸의 디코드 실패 통보 — 텍스트 폴백으로 전환하고 재요청 루프를 끊는다.
    pub(crate) fn set_preview_failed(&mut self, id: u64) {
        self.preview_img = Some((id, None));
        self.redraw();
    }

    /// ★ 미리보기 열림 상태 적용(토글 셸 왕복 · 09-02 K4).
    pub(crate) fn apply_preview(&mut self, on: bool) {
        self.preview_open = on;
        self.sync_preview();
        self.redraw();
    }

    /// 창 이벤트 처리 — 행동은 셸로 되돌린다.
    pub(crate) fn handle_event(&mut self, event: &WindowEvent) -> MainAction {
        self.sync_preview();
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
                self.alt = m.state().alt_key();
                self.primary = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if self.feed_bars(event) {
                    return MainAction::None;
                }
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
                // ★ 패널 위 휠 = 미리보기 전문 스크롤(09-02 K4) — 목록은 건드리지 않는다.
                if self.preview_open {
                    if let Some(win) = &self.window {
                        let sz = win.inner_size();
                        let pr = self.preview_rect(sz.width as i32, sz.height as i32);
                        let (cx, cy) = self.cursor;
                        if pr.contains(nclip_ctl::geom::Point { x: cx, y: cy }) {
                            if let (Some((_, tb)), Some(ev)) =
                                (self.preview_tb.as_mut(), to_ctl_event(event, self.cursor))
                            {
                                let mut inv = Invalidations::default();
                                tb.on_event(&ev, &mut inv);
                                if !inv.is_empty() {
                                    self.redraw();
                                }
                            }
                            return MainAction::None;
                        }
                    }
                }
                // ★ 픽셀 스크롤(09-02 실기) — 노치 1 = 3행 상당 · 패드 픽셀 델타는 그대로.
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * (self.row_h() * 3) as f32,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                };
                self.wheel_frac += dy;
                #[allow(clippy::cast_possible_truncation)]
                let d = self.wheel_frac as i32;
                if d != 0 && !self.rows.is_empty() {
                    self.wheel_frac -= d as f32;
                    self.scroll += d; // 상한은 paint에서 죈다(rows 길이×행 높이).
                    self.bars.show();
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.feed_bars(event) {
                    return MainAction::None;
                }
                let (sx, sy) = self.cursor;
                // ★ 검색 우클릭 = 편집 메뉴(09-02) · 메뉴 열림 동안은 전부 검색 몫.
                if *state == ElementState::Pressed
                    && *button == winit::event::MouseButton::Right
                    && self
                        .search
                        .bounds()
                        .contains(nclip_ctl::geom::Point { x: sx, y: sy })
                {
                    self.search
                        .set_clipboard_has_text(crate::cliptext::has_text());
                    let mut inv = Invalidations::default();
                    self.search
                        .on_event(&CtlEvent::RightDown { x: sx, y: sy }, &mut inv);
                    self.redraw();
                    return MainAction::None;
                }
                if self.search.popup_open() {
                    let ev = match (state, button) {
                        (ElementState::Pressed, winit::event::MouseButton::Left) => {
                            Some(CtlEvent::MouseDown {
                                x: sx,
                                y: sy,
                                shift: self.shift,
                                primary: self.primary,
                            })
                        }
                        (ElementState::Released, winit::event::MouseButton::Left) => {
                            Some(CtlEvent::MouseUp { x: sx, y: sy })
                        }
                        _ => None,
                    };
                    if let Some(ev) = ev {
                        let mut inv = Invalidations::default();
                        self.search.on_event(&ev, &mut inv);
                        self.drain_search_edit_ctx();
                        self.redraw();
                        return MainAction::QueryChanged;
                    }
                    return MainAction::None;
                }
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
                    let before = self.search.display_text();
                    let mut inv = Invalidations::default();
                    self.search.on_event(&CtlEvent::MouseUp { x, y }, &mut inv);
                    // ×(지우기) 클릭 등으로 값이 바뀌었으면 필터 재적용.
                    if self.search.display_text() != before {
                        return MainAction::QueryChanged;
                    }
                }
                if *state == ElementState::Pressed && *button == winit::event::MouseButton::Left {
                    let (x, y) = self.cursor;
                    if self
                        .search
                        .bounds()
                        .contains(nclip_ctl::geom::Point { x, y })
                    {
                        // ★ ×(지우기)는 MouseDown에서 값이 바뀐다(09-02 실기 — 빈 결과
                        //   상태에서 × 눌러도 목록이 안 돌아오던 원인) → 변화 감지해 재필터.
                        let before = self.search.display_text();
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
                        if self.search.display_text() != before {
                            return MainAction::QueryChanged;
                        }
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
                            crate::cliptext::set_text(&t);
                        }
                    }
                    Key::Character("x" | "X") if self.primary => {
                        let mut inv = Invalidations::default();
                        if let Some(t) = self.search.cut_selection(&mut inv) {
                            crate::cliptext::set_text(&t);
                            return MainAction::QueryChanged;
                        }
                    }
                    Key::Character("v" | "V") if self.primary => {
                        if let Some(t) = crate::cliptext::get_text() {
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
        let l_h = (h - self.header_h() - self.status_h() - self.preview_h_of(h)).max(1);
        if self.sel + 1 >= self.row_offs.len() {
            return; // 오프셋 미생성(첫 paint 전) — 다음 paint가 재계산.
        }
        let (top, bot) = (self.row_offs[self.sel], self.row_offs[self.sel + 1]);
        if top < self.scroll {
            self.scroll = top;
        } else if bot > self.scroll + l_h {
            self.scroll = bot - l_h;
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
        {
            let l = self.list_rect(w, h);
            self.rebuild_offsets(l.w);
            let total = *self.row_offs.last().unwrap_or(&0);
            self.scroll = self.scroll.clamp(0, (total - l.h).max(0));
        }
        {
            let pad = (self.scale * 10.0).round() as i32;
            let mut inv = Invalidations::default();
            self.search.set_scale(self.scale);
            let tbw = self.toolbar_w();
            self.search.set_bounds(
                Rect::new(
                    tbw + pad,
                    (self.scale * 7.0).round() as i32,
                    (w - tbw - pad * 2).max(40),
                    (self.scale * 24.0).round() as i32,
                ),
                &mut inv,
            );
        }
        if let Some((_, tb)) = self.editor.as_mut() {
            let tbw = (self.scale * 40.0).round() as i32;
            let hh = (self.scale * 38.0).round() as i32;
            let sh = (self.scale * 22.0).round() as i32;
            let pad = (self.scale * 10.0).round() as i32;
            let bar = (self.scale * 26.0).round() as i32; // 줄 바꿈 스위치 줄(09-02 M4)
            let list = Rect::new(
                tbw + pad,
                hh + pad + bar,
                (w - tbw - pad * 2).max(60),
                (h - hh - sh - pad * 2 - bar - (self.scale * 20.0) as i32).max(60),
            );
            let mut inv = Invalidations::default();
            tb.set_bounds(list, &mut inv);
            if let Some(sw) = self.wrap_sw.as_mut() {
                let sww = (self.scale * 40.0).round() as i32;
                sw.set_bounds(Rect::new(w - pad - sww, hh + pad, sww, bar - 4), &mut inv);
            }
        }
        let pr = self.preview_rect(w, h);
        if let Some((_, tb)) = self.preview_tb.as_mut() {
            let pad2 = (self.scale * 8.0).round() as i32;
            let mut inv = Invalidations::default();
            tb.set_scale(self.scale);
            tb.set_bounds(
                Rect::new(
                    pr.x + pad2,
                    pr.y + pad2,
                    (pr.w - pad2 * 2).max(20),
                    (pr.h - pad2 * 2).max(20),
                ),
                &mut inv,
            );
        }
        // ★ surface를 잠시 꺼내 빌림을 끕는다 — draw(&self)가 전체 상태를 읽기 때문.
        let Some(mut surface) = self.surface.take() else {
            return;
        };
        if surface.resize(nw, nh).is_ok() {
            if let Ok(mut buf) = surface.buffer_mut() {
                {
                    let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
                    let mut dc = RasterCtx::new(&mut gfx, &self.font, self.scale)
                        .with_caret_on(self.caret_phase);
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

        // ── ② 좌측 세로 툴바 — ★ 전고(09-02 사용자 — 툴바가 먼저 서고
        //    검색은 우측 영역 상단만 차지) ──
        let header_h = self.header_h();
        let pad = px(10.0);
        let tb_w = self.toolbar_w();
        dc.fill_rect(Rect::new(0, 0, tb_w, h - self.status_h()), th.chrome_bg);
        dc.fill_rect(Rect::new(tb_w - 1, 0, 1, h - self.status_h()), th.border);

        // ── ① 검색 1줄 — 툴바 오른쪽(정식 TextBox · 09-01) ──
        dc.fill_rect(Rect::new(tb_w, 0, w - tb_w, header_h), th.chrome_bg);
        self.search.paint(dc, &th);
        dc.fill_rect(Rect::new(tb_w, header_h - 1, w - tb_w, 1), th.border);
        let has_sel = !self.rows.is_empty();
        for (k, t) in TOOLS_TOP.iter().enumerate() {
            match t {
                Some(t) => {
                    self.draw_tool(dc, self.tool_rect(k), *t, has_sel || *t == Tool::Preview)
                }
                None => {
                    let r = self.tool_rect(k);
                    dc.fill_rect(
                        Rect::new(r.x + px(4.0), r.y + px(3.0), r.w - px(8.0), 1),
                        th.border,
                    );
                }
            }
        }
        self.draw_tool(dc, self.always_top_rect(h), Tool::AlwaysTop, true);
        self.draw_tool(dc, self.settings_rect(h), Tool::Settings, true);

        // ── ③ 목록(핀 구획 먼저) ──
        let list = self.list_rect(w, h);
        let row_h = self.row_h();
        if self.rows.is_empty() {
            let lang = current_lang();
            let msg = if self.search.display_text().is_empty() {
                tr(lang, Msg::MainNoItems)
            } else {
                tr(lang, Msg::MainNoMatch)
            };
            dc.text(list.x + pad, list.y + px(12.0), full, msg, th.text_dim);
        }
        let mut pin_divider_done = false;
        // ★ 가변 행(누적합) — 첫 행은 이분 탐색, 밑으로 나가면 중단(09-02).
        let first = self
            .row_offs
            .partition_point(|&o| o <= self.scroll)
            .saturating_sub(1);
        for (vi, row) in self.rows.iter().enumerate().skip(first) {
            let y = list.y - self.scroll + self.row_offs[vi];
            if y >= list.y + list.h {
                break;
            }
            let rh = self
                .row_offs
                .get(vi + 1)
                .map_or(row_h, |e| e - self.row_offs[vi]);
            let cy0 = y.max(list.y);
            let clip = Rect::new(
                list.x,
                cy0,
                list.w,
                ((y + rh).min(list.y + list.h) - cy0).max(0),
            );
            if vi == self.sel {
                dc.fill_rect(clip, th.sel_bg);
            } else if vi % 2 == 1 {
                dc.fill_rect(clip, th.panel_bg_alt);
            }
            // 핀 구획 경계 — 첫 비고정 행 위에 한 줄.
            if !pin_divider_done && !row.pinned && vi > 0 {
                // ★ 부분 행이면 경계선은 화면 밖 — 옮겨 그리지 않는다(09-02 실기).
                if y >= list.y {
                    dc.fill_rect(Rect::new(list.x, y, list.w, 1), th.accent);
                }
                pin_divider_done = true;
            }
            let tx = list.x + pad;
            // ★ Rich = CopyQ 화법 전면(09-02 사용자 — "CopyQ처럼 보이게"):
            //   거터(번호·핀·×n)만 남기고 행 = **내용 그 자체**.
            //   이미지/EMF는 렌더 결과를 행 높이로 · 텍스트는 본문 3줄(본문 색).
            //   라벨·출처 줄은 접는다(서식 렌더러는 T-18d).
            if self.view == ViewMode::Rich {
                dc.select_font(FontSlot::Status, false);
                let no = format!("{}", vi + 1);
                dc.text(tx, y + px(6.0), clip, &no, th.text_dim);
                dc.select_font(FontSlot::Base, false);
                if row.pinned {
                    let dot_y = y + px(22.0);
                    if dot_y >= clip.y && dot_y + px(6.0) <= clip.y + clip.h {
                        dc.fill_round_rect(
                            Rect::new(tx, dot_y, px(6.0), px(6.0)),
                            px(3.0),
                            th.accent,
                        );
                    }
                }
                let mut right = list.x + list.w - pad;
                if row.copies > 1 {
                    let tag = format!("×{}", row.copies);
                    let tw = dc.text_width(&tag);
                    right -= tw;
                    dc.text(right, y + px(6.0), clip, &tag, th.text_dim);
                    right -= px(8.0);
                }
                let cx0 = tx + px(30.0);
                let content_clip = Rect::new(cx0, clip.y, (right - cx0).max(0), clip.h);
                if let Some(img) = &row.thumb {
                    // ★ 원본 치수 기준 — 복사본마다 확대율이 같다(최대 높이만 비율 축소).
                    #[allow(clippy::cast_possible_wrap)]
                    let (ow, oh) = row
                        .img_dims
                        .map_or((img.w.max(1) as i32, img.h.max(1) as i32), |(a, b)| {
                            (a as i32, b as i32)
                        });
                    let (dw, dh) = self.rich_fit(ow, oh, self.content_w_of(list.w));
                    let dst = Rect::new(cx0, y + px(6.0), dw, dh);
                    dc.image_scaled(dst, img, content_clip);
                } else {
                    let text = row.plain.as_deref().unwrap_or(row.label.as_str());
                    for (k, line) in text.lines().take(5).enumerate() {
                        let one: String = line.chars().take(200).collect();
                        #[allow(clippy::cast_precision_loss)]
                        dc.text(
                            cx0,
                            y + px(6.0 + 22.0 * k as f32),
                            content_clip,
                            &one,
                            th.text,
                        );
                    }
                }
                continue;
            }
            // 보기 3모드(docs/04 §2-2) — Plain은 글리프도 접어 밀도 최우선.
            let show_glyph = self.view != ViewMode::Plain;
            if show_glyph {
                if let Some(img) = &row.thumb {
                    let box_side = px(24.0);
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
            let text_y = if self.view == ViewMode::Plain {
                y + px(3.0)
            } else {
                y + px(7.0)
            };
            let mut lx = tx
                + if self.view == ViewMode::Plain {
                    px(0.0)
                } else {
                    px(30.0)
                };
            if row.pinned {
                // ★ fill_round_rect는 clip을 모른다 — 부분 행에서 검색바 위로 샐던
                //   하늘색 점(09-02 실기) → 점 전체가 행 clip 안일 때만 그린다.
                let dot_y = text_y + px(4.0);
                if dot_y >= clip.y && dot_y + px(6.0) <= clip.y + clip.h {
                    dc.fill_round_rect(Rect::new(lx, dot_y, px(6.0), px(6.0)), px(3.0), th.accent);
                }
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
            if !row.source.is_empty() {
                let tw = dc.text_width(&row.source);
                right -= tw;
                dc.text(right, text_y, clip, &row.source, th.text_dim);
                right -= px(8.0);
            }
            // ★ 세로는 행 clip을 따른다 — 부분 행이 검색바/패널을 침범하지 않게(09-02).
            let label_clip = Rect::new(lx, clip.y, (right - lx).max(0), clip.h);
            dc.text(lx, text_y, label_clip, &row.label, th.text);
            // ★ 이미지·개체 항목 식별 보조(09-02 Ctrl+2 — "[이미지] W×H"만으로는 무엇을
            //   복사했는지 모른다) — 라벨 뒤에 본문 첫 줄을 흐릿하게 이어 붙인다.
            if matches!(row.kind, ClipKind::Image | ClipKind::Object) {
                if let Some(first) = row.plain.as_deref().and_then(|pl| pl.lines().next()) {
                    if !first.trim().is_empty() {
                        let snippet: String = first.chars().take(120).collect();
                        let sx = lx + dc.text_width(&row.label) + px(8.0);
                        dc.text(sx, text_y, label_clip, &snippet, th.text_dim);
                    }
                }
            }
        }

        // 목록 오버레이 스크롤바(자동 숨김 · 설정창과 동일 화법).
        self.bars.paint(
            dc,
            &th,
            list,
            list.w,
            *self.row_offs.last().unwrap_or(&0),
            0,
            self.scroll,
            self.scale,
        );

        // ── ③b 미리보기 패널(09-02 K4) — 텍스트 전문(wrap·휠) / 이미지 원본(비율 유지) ──
        if self.preview_open {
            let pr = self.preview_rect(w, h);
            dc.fill_rect(pr, th.panel_bg);
            dc.fill_rect(Rect::new(pr.x, pr.y, pr.w, 1), th.border);
            let sel_id = self.rows.get(self.sel).map(|r| r.id);
            if let Some((id, Some(img))) = &self.preview_img {
                if Some(*id) == sel_id {
                    let pad2 = px(8.0);
                    let inner = Rect::new(
                        pr.x + pad2,
                        pr.y + pad2,
                        (pr.w - pad2 * 2).max(1),
                        (pr.h - pad2 * 2).max(1),
                    );
                    let (iw, ih) = (img.w.max(1) as i32, img.h.max(1) as i32);
                    // 축소만 — 원본보다 키우면 계단이 진다.
                    let (dw, dh) = if iw <= inner.w && ih <= inner.h {
                        (iw, ih)
                    } else if iw * inner.h >= ih * inner.w {
                        (inner.w, (inner.w * ih / iw).max(1))
                    } else {
                        ((inner.h * iw / ih).max(1), inner.h)
                    };
                    let dst = Rect::new(
                        inner.x + (inner.w - dw) / 2,
                        inner.y + (inner.h - dh) / 2,
                        dw,
                        dh,
                    );
                    dc.image_scaled(dst, img, inner);
                }
            } else if let Some((_, tb)) = &self.preview_tb {
                tb.paint(dc, &th);
            }
        }

        // ── ④ 상태 1줄 ──
        let sy = h - self.status_h();
        dc.fill_rect(Rect::new(0, sy, w, self.status_h()), th.chrome_bg);
        dc.fill_rect(Rect::new(0, sy, w, 1), th.border);
        let lang = current_lang();
        let status = if self.search.display_text().is_empty() {
            tr(lang, Msg::StatusLine).replacen("{}", &self.total.to_string(), 1)
        } else {
            tr(lang, Msg::StatusLineFiltered)
                .replacen("{}", &self.rows.len().to_string(), 1)
                .replacen("{}", &self.total.to_string(), 1)
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
            if let Some(sw) = &self.wrap_sw {
                let r = sw.bounds();
                dc.select_font(FontSlot::Status, false);
                let label = tr(current_lang(), Msg::WrapLabel);
                let tw = dc.text_width(label);
                dc.text(r.x - tw - px(8.0), r.y + px(5.0), full, label, th.text_dim);
                dc.select_font(FontSlot::Base, false);
                sw.paint(dc, &th);
            }
            dc.select_font(FontSlot::Status, false);
            dc.text(
                list.x + px(10.0),
                list.y + list.h - px(18.0),
                full,
                tr(current_lang(), Msg::EditorHint),
                th.text_dim,
            );
            dc.select_font(FontSlot::Base, false);
        }
        // 검색 우클릭 편집 메뉴 — 맨 위 레이어.
        self.search.paint_popup(dc, &th);
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
        if tool == Tool::AlwaysTop {
            return self.always_top_rect(h);
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
            Tool::Preview => {
                // Material `visibility` — 눈 윤곽(타원 링) + 홍채. 켜짐 = accent.
                let c = if self.preview_open { th.accent } else { ink };
                dc.fill_ellipse(Rect::new(cx - px(9.0), cy - px(5.5), px(18.0), px(11.0)), c);
                dc.fill_ellipse(
                    Rect::new(cx - px(7.0), cy - px(3.8), px(14.0), px(7.6)),
                    th.chrome_bg,
                );
                dc.fill_ellipse(Rect::new(cx - px(3.0), cy - px(3.0), px(6.0), px(6.0)), c);
            }
            Tool::AlwaysTop => {
                // ★ Material `layers`(09-02 사용자 시안) — 꺼짐 = 윗장 윤곽선 + 밴드 1,
                //   켜짐 = accent 적층 3장. 마름모 = 삼각형 2장 합성.
                let on = self.always_top;
                let c = if on { th.accent } else { ink };
                let (w2, h2) = (px(8.0), px(4.5));
                let mut rhombus = |dy: f32, col: nclip_ctl::theme::Color| {
                    let yc = cy + px(dy);
                    dc.fill_triangle((cx, yc - h2), (cx - w2, yc), (cx + w2, yc), col);
                    dc.fill_triangle((cx - w2, yc), (cx + w2, yc), (cx, yc + h2), col);
                };
                if on {
                    // 아래서부터 밴드 2장(아래 V만 남기고 바탕으로 뒸어냄) + 윗장 꽉 채움.
                    rhombus(5.0, c);
                    rhombus(3.0, th.chrome_bg);
                    rhombus(2.0, c);
                    rhombus(0.0, th.chrome_bg);
                    rhombus(-3.0, c);
                } else {
                    rhombus(2.5, c);
                    rhombus(0.5, th.chrome_bg);
                    rhombus(-3.0, c);
                    // 윗장 속을 바탕색으로 비워 윤곽선만 남긴다.
                    let yc = cy + px(-3.0);
                    let (iw, ih2) = (px(5.2), px(2.9));
                    dc.fill_triangle((cx, yc - ih2), (cx - iw, yc), (cx + iw, yc), th.chrome_bg);
                    dc.fill_triangle((cx - iw, yc), (cx + iw, yc), (cx, yc + ih2), th.chrome_bg);
                }
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
        let lang = current_lang();
        let mut items = vec![
            CtxItem::item("copy", tr(lang, Msg::MenuCopy)),
            CtxItem::item("plain", tr(lang, Msg::MenuCopyPlain)),
        ];
        if row.kind == ClipKind::Files {
            items.push(CtxItem::item("object", tr(lang, Msg::MenuCopyObject)));
            items.push(CtxItem::item("path", tr(lang, Msg::MenuCopyPath)));
        }
        items.push(CtxItem::item(
            "pin",
            if row.pinned {
                tr(lang, Msg::MenuUnpin)
            } else {
                tr(lang, Msg::MenuPin)
            },
        ));
        let editable = matches!(row.kind, ClipKind::Text | ClipKind::RichText);
        items.push(CtxItem::maybe("edit", tr(lang, Msg::MenuEdit), editable));
        items.push(CtxItem::item("delete", tr(lang, Msg::MenuDelete)));
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
        tb.set_wrap(true); // ★ 기본 줄 바꿈(09-02) — Alt+Z 또는 우상단 스위치.
        tb.set_scale(self.scale);
        tb.set_focused(true);
        let mut sw = Switch::new("", true).with_label_side(LabelSide::None);
        sw.set_scale(self.scale);
        self.wrap_sw = Some(sw);
        self.editor = Some((id, tb));
        self.redraw();
    }

    /// 편집 중 이벤트 — Esc 취소 · Ctrl+Enter 저장 · 나머지는 입력 상자로.
    fn handle_editor_event(&mut self, event: &WindowEvent) -> MainAction {
        match event {
            WindowEvent::CloseRequested => {
                self.editor = None;
                self.wrap_sw = None;
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
                self.alt = m.state().alt_key();
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
                        self.wrap_sw = None;
                        self.redraw();
                        return MainAction::None;
                    }
                    Key::Named(NamedKey::Enter) if self.primary => {
                        if let Some((id, tb)) = self.editor.take() {
                            self.wrap_sw = None;
                            self.redraw();
                            return MainAction::SaveEdit {
                                id,
                                text: tb.text(),
                            };
                        }
                        return MainAction::None;
                    }
                    // ★ Alt+Z = 줄 바꿈 토글(09-02 · VS Code 관례).
                    Key::Character("z" | "Z") if self.alt_down(kev) => {
                        if let Some((_, tb)) = self.editor.as_mut() {
                            let on = !tb.wrap();
                            tb.set_wrap(on);
                            if let Some(sw) = self.wrap_sw.as_mut() {
                                sw.set_on(on);
                            }
                            self.redraw();
                        }
                        return MainAction::None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // 우상단 줄 바꿈 스위치 — 마우스 사건을 먼저 준다(놓음으로 확정 · Button 계약).
        if let Some(ev) = to_ctl_event(event, self.cursor) {
            if matches!(
                ev,
                CtlEvent::MouseDown { .. } | CtlEvent::MouseUp { .. } | CtlEvent::MouseMove { .. }
            ) {
                if let Some(sw) = self.wrap_sw.as_mut() {
                    let mut inv = Invalidations::default();
                    sw.on_event(&ev, &mut inv);
                    if let Some(on) = sw.take_toggled() {
                        if let Some((_, tb)) = self.editor.as_mut() {
                            tb.set_wrap(on);
                        }
                        self.redraw();
                        return MainAction::None;
                    }
                    if !inv.is_empty() {
                        self.redraw();
                    }
                }
            }
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
pub(crate) fn to_ctl_event(event: &WindowEvent, cursor: (i32, i32)) -> Option<CtlEvent> {
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
/// 평문 표현을 순위대로 고른다(스냅숏 `plain_text`와 같은 계약 · 09-02 실기 P —
/// "첫 디코드 성공"이 CF_HTML 헤더·벤더 바이트를 물어 목록/미리보기에 깨진 글이 떴다).
fn plain_of(reps: &[nclip_core::RawRep]) -> Option<String> {
    let mut best: Option<(u8, &nclip_core::RawRep)> = None;
    for r in reps {
        if let Some(rank) = nclip_core::capture::plain_rank(&r.format) {
            if best.is_none_or(|(b, _)| rank < b) {
                best = Some((rank, r));
            }
        }
    }
    let (_, r) = best?;
    decode_plain(&r.format, &r.data)
}

/// ★ 표시 치수 = **문서 논리 크기**(09-02 사용자 — "CopyQ 배율 차용"):
/// PPT 래스터(PNG/DIB)는 ~150dpi 렌더라 화소 수 그대로 그리면 실물보다 ~1.56배
/// 크다. CopyQ(Qt)는 SVG **선언 크기**(예: 447×51)로 그려 실물과 비슷하다.
/// 우선순위: SVG width/height → EMF 프레임(0.01mm→96dpi) → 없음(래스터 화소 폴백).
fn display_dims(reps: &[nclip_core::RawRep]) -> Option<(u32, u32)> {
    if let Some(r) = reps.iter().find(|r| r.format.starts_with("image/svg")) {
        if let Ok(xml) = std::str::from_utf8(&r.data) {
            if let (Some(w), Some(h)) = (svg_attr(xml, "width"), svg_attr(xml, "height")) {
                return Some((w, h));
            }
        }
    }
    if let Some(r) = reps
        .iter()
        .find(|r| r.format == "CF_ENHMETAFILE" && r.data.len() >= 40)
    {
        let g = |o: usize| i32::from_le_bytes(r.data[o..o + 4].try_into().unwrap_or_default());
        let (w01, h01) = (i64::from(g(32) - g(24)), i64::from(g(36) - g(28)));
        let (w, h) = (
            (w01 * 96 / 2540).max(0) as u32,
            (h01 * 96 / 2540).max(0) as u32,
        );
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    None
}

/// 첫 `<svg>` 태그의 수치 속성(px 단위 가정 · `"100%"` 등은 실패 = 폴백).
fn svg_attr(xml: &str, name: &str) -> Option<u32> {
    let i = xml.find("<svg")?;
    let tag = &xml[i..i + xml[i..].find('>')?];
    let pat = format!("{name}=\"");
    let j = tag.find(&pat)? + pat.len();
    let v: f32 = tag[j..j + tag[j..].find('"')?]
        .trim_end_matches("px")
        .parse()
        .ok()?;
    (v >= 1.0).then(|| v.round() as u32)
}

/// 라벨의 "W×H" 치수 파싱(언어 무관 — × 양쪽 숫자만 본다) — 논리 크기를 못 얻는
/// 항목(순수 래스터)의 폴백(09-02 가변 행).
fn parse_dims(label: &str) -> Option<(u32, u32)> {
    let x = label.find('×')?;
    let w: u32 = label[..x]
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .ok()?;
    let h: u32 = label[x + '×'.len_utf8()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

use nclip_core::capture::svg_text;
fn tool_label(t: Tool) -> &'static str {
    let lang = current_lang();
    match t {
        Tool::Pin => tr(lang, Msg::TipPin),
        Tool::Delete => tr(lang, Msg::TipDelete),
        Tool::Copy => tr(lang, Msg::TipCopy),
        Tool::CopyPlain => tr(lang, Msg::TipCopyPlain),
        Tool::Preview => tr(lang, Msg::TipPreview),
        Tool::AlwaysTop => tr(lang, Msg::TipAlwaysTop),
        Tool::Settings => tr(lang, Msg::TraySettings),
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
