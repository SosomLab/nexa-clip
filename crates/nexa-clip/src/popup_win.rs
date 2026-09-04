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
use nclip_ctl::ViewMode;
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
    /// ★ 붙여넣기 스택(09-03 ③ — Ditto 화법): 표시 순서대로 **순차** 붙여넣기.
    PickStack(Vec<u64>),
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
    /// 항목 id — ★ 스택 표시가 인덱스 흔들림 없이 붙게(09-03 ③).
    id: u64,
    /// ★ 핀 — 메인창과 같은 구획·표시(09-02 사용자 "메인과 동일하게").
    pinned: bool,
    kind: ClipKind,
    label: String,
    copies: u32,
    /// ★ 다른 기기에서 받은 항목(09-04) — 번호 아래 녹색 점(메인창과 동일).
    remote: bool,
    /// 내용 열쇠(병합) · 출처(메타).
    key: u64,
    origin: Option<String>,
    /// ★ 섬네일 치수(09-04 · 30 §4) — 화소는 공유 캐시에서 그릴 때 꺼낸다.
    thumb_dims: Option<(u32, u32)>,
    /// 본문(평문 순위 → SVG 추출) — Rich 본문·이미지 라벨 뒤 식별 보조(09-02).
    plain: Option<String>,
    /// 표시 치수(문서 논리 크기 · 09-02 — 메인과 같은 배율 규약).
    img_dims: Option<(u32, u32)>,
    /// ★ 리치 런(T-18d 1단 · 메인과 동일).
    rich: Option<Vec<Vec<nclip_core::richtext::Run>>>,
}

pub(crate) struct Popup {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    /// ★ 고정폭 글꼴(09-04) — 리치 런 Mono 슬롯.
    font_mono: Option<Font>,
    /// ★ 섬네일 공유 캐시(09-04 · 30 §4).
    thumbs: Option<crate::thumbs::Thumbs>,
    theme: Theme,
    scale: f32,
    /// ★ 검색 입력(09-02 — "메인 검색창의 모든 기능을 팝업에도") — 이식 `TextBox`
    ///   정식 편집기: 캐럿·드래그 선택·×지우기·우클릭 편집 메뉴·IME preedit.
    search: TextBox,
    /// 캐럿 깜박임 위상(셸이 500ms마다 돌린다).
    caret_phase: bool,
    /// 필터 통과 행(최신이 위).
    rows: Vec<Row>,
    /// ★ 병합 보기(09-04) — 메인창 설정과 동일.
    dedup: bool,
    /// 선택(rows 인덱스).
    sel: usize,
    /// ★ 목록 세로 스크롤(**픽셀** · 09-02 실기 — 메인과 동일 규약).
    scroll: i32,
    /// 휠 소수 델타 누적기(터치패드).
    wheel_frac: f32,
    /// ★ 목록 오버레이 스크롤바(자동 숨김 · 09-02).
    bars: ScrollBars,
    /// ★ 보기 모드(`ui.popup_view` · 기본 Rich — 09-02 사용자 확정).
    view: ViewMode,
    /// ★ 스택 선택(id · 선택 순서 유지 — 09-03 ③): Enter = 순서대로 붙여넣기.
    marked: Vec<u64>,
    /// ★ 열 때 쓸 크기(물리 px · `ui.popup_w/h` — 09-02 "마지막 크기 기억").
    pref_size: Option<(u32, u32)>,
    /// 마지막 실측 크기(Resized에서 갱신) — 닫을 때 셸이 저장.
    last_size: Option<(u32, u32)>,
    /// 행 시작 y 누적합(len = rows+1) — Rich 가변 행(메인과 같은 규약).
    row_offs: Vec<i32>,
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
    /// ★ 검색 색인(09-04) — id → 소문자 검색문(셸이 채운다 · 없으면 라벨로).
    search_idx: Option<crate::search_index::SearchIndex>,
    /// ★ 검색 방식(09-04 · 설정 `find.mode`).
    search_mode: nclip_core::search::Mode,
    /// ★ 행 hover 페이드(09-04 사용자 — 메인과 동일): 의도 코얼레싱(70ms · 휠 안정 120ms) + 상태 레이어 6%.
    row_fade: nclip_ctl::tokens::HoverFade,
    row_intent: nclip_ctl::tokens::HoverIntent<usize>,
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
fn clamp_to_monitor(el: &ActiveEventLoop, x: i32, y: i32, pref: Option<(u32, u32)>) -> (i32, i32) {
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
    // ★ 저장된 크기가 있으면 그걸로 클램프(09-02 — 리사이즈 기억과 짝).
    #[allow(clippy::cast_possible_wrap)]
    let (pw, ph) = pref.map_or_else(
        || {
            (
                (POPUP_W * scale).ceil() as i32,
                (POPUP_H * scale).ceil() as i32,
            )
        },
        |(w, h)| (w as i32, h as i32),
    );
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
    /// ★ 고정폭 글꼴 주입(09-04).
    pub(crate) fn set_mono_font(&mut self, font: Option<Font>) {
        self.font_mono = font;
    }

    /// ★ 섬네일 캐시 주입(09-04 · 30 §4).
    pub(crate) fn set_thumbs(&mut self, thumbs: crate::thumbs::Thumbs) {
        self.thumbs = Some(thumbs);
    }

    /// ★ 검색 색인 주입(09-04).
    pub(crate) fn set_search_index(&mut self, idx: crate::search_index::SearchIndex) {
        self.search_idx = Some(idx);
    }

    /// ★ 검색 방식 동기(09-04) — 바뀌었을 때만 true.
    pub(crate) fn set_search_mode(&mut self, mode: nclip_core::search::Mode) -> bool {
        if self.search_mode == mode {
            return false;
        }
        self.search_mode = mode;
        true
    }

    /// 셸이 캐시를 채운 뒤 — 창이 있으면 다시 그린다.
    pub(crate) fn redraw_now(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub(crate) fn new(font: Font) -> Self {
        Self {
            window: None,
            ctx: None,
            surface: None,
            font,
            font_mono: None,
            thumbs: None,
            theme: Theme::dark(),
            scale: 1.0,
            search: {
                let mut t = TextBox::new(tr(current_lang(), Msg::SearchHint)).with_clearable();
                t.set_focused(true);
                t
            },
            caret_phase: true,
            rows: Vec::new(),
            dedup: true,
            sel: 0,
            scroll: 0,
            wheel_frac: 0.0,
            bars: ScrollBars::new(),
            view: ViewMode::Rich,
            marked: Vec::new(),
            pref_size: None,
            last_size: None,
            row_offs: Vec::new(),
            shift: false,
            ctrl: false,
            alt: false,
            was_focused: false,
            opened_at: std::time::Instant::now(),
            cursor: (0, 0),
            search_mode: nclip_core::search::Mode::Exact,
            search_idx: None,
            row_fade: nclip_ctl::tokens::HoverFade::default(),
            row_intent: nclip_ctl::tokens::HoverIntent::default(),
        }
    }

    /// 좌표 → 목록 행(rows 인덱스) — 그리기와 같은 자(scale 기반)를 쓴다.
    fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        let Some(win) = &self.window else { return None };
        let size = win.inner_size();
        let px = |v: f32| (v * self.scale).round() as i32;
        let (header_h, footer_h) = (px(38.0), px(24.0));
        let list_bot = size.height as i32 - footer_h;
        if x < 0 || x >= size.width as i32 || y < header_h || y >= list_bot {
            return None;
        }
        let ly = y - header_h + self.scroll.max(0);
        if self.row_offs.len() < 2 || ly >= *self.row_offs.last().unwrap_or(&0) {
            return None;
        }
        let vi = self
            .row_offs
            .partition_point(|&o| o <= ly)
            .saturating_sub(1);
        (vi < self.rows.len()).then_some(vi)
    }

    /// 행 높이(px) — 모드별 명목치(Compact·Plain은 그대로 행 높이 · Rich는 휠 노치 기준).
    fn row_h(&self) -> i32 {
        let pt = match self.view {
            ViewMode::Rich => 76.0,
            ViewMode::Compact => 30.0,
            ViewMode::Plain => 22.0,
        };
        ((self.scale * pt).round() as i32).max(1)
    }

    /// ★ 보기 모드 적용(`ui.popup_view` — 열 때마다 셸이 읽어 넘긴다 · 09-02).
    /// 선택 행을 화면 안으로 — 키 이동 몷(09-02 · paint 스냅 제거의 짝).
    /// ★ 스택 표시 토글(09-03 ③) — 이미 있으면 빼고, 없으면 **선택 순서대로** 뒤에 붙인다.
    fn toggle_mark(&mut self, vi: usize) {
        let Some(row) = self.rows.get(vi) else { return };
        let id = row.id;
        if let Some(pos) = self.marked.iter().position(|&m| m == id) {
            self.marked.remove(pos);
        } else {
            self.marked.push(id);
        }
        self.redraw();
    }

    fn ensure_visible(&mut self) {
        let Some(win) = &self.window else { return };
        if self.sel + 1 >= self.row_offs.len() {
            return;
        }
        let px = |v: f32| (v * self.scale).round() as i32;
        let list_h = ((win.inner_size().height as i32 - px(24.0)) - px(38.0)).max(1);
        let (top, bot) = (self.row_offs[self.sel], self.row_offs[self.sel + 1]);
        if top < self.scroll {
            self.scroll = top;
        } else if bot > self.scroll + list_h {
            self.scroll = bot - list_h;
        }
    }

    /// ★ 병합(중복 제외) 보기 — 메인창 설정(`ui.dedup_view`)을 그대로 따른다(09-04).
    pub(crate) fn set_dedup(&mut self, on: bool) {
        self.dedup = on;
    }

    pub(crate) fn set_view_code(&mut self, code: &str) {
        self.view = ViewMode::from_code(code).unwrap_or(ViewMode::Rich);
    }

    /// ★ 저장된 크기(물리 px) — 열기 전에 셸이 넣는다(09-02 "마지막 크기 기억").
    pub(crate) fn set_pref_size(&mut self, wh: Option<(u32, u32)>) {
        self.pref_size = wh;
    }

    /// 마지막 실측 크기 — 닫을 때 셸이 저장한다.
    pub(crate) fn last_size(&self) -> Option<(u32, u32)> {
        self.last_size
    }

    /// Rich 이미지 표시 크기 — 메인과 같은 규약(논리 크기 64% · 폭/최대 200px 비율 축소).
    fn rich_fit(&self, ow: i32, oh: i32, content_w: i32) -> (i32, i32) {
        let px = |v: f32| (v * self.scale).round() as i32;
        let max_h = px(200.0);
        let (ow, oh) = ((ow * 16 / 25).max(1), (oh * 16 / 25).max(1));
        let mut dw = ow.min(content_w.max(40));
        let mut dh = (oh * dw / ow).max(1);
        if dh > max_h {
            dw = (dw * max_h / dh).max(1);
            dh = max_h;
        }
        (dw, dh)
    }

    fn content_w_of(&self, w: i32) -> i32 {
        let px = |v: f32| (v * self.scale).round() as i32;
        (w - px(10.0) * 2 - px(30.0)).max(40)
    }

    fn row_height_of(&self, row: &Row, content_w: i32) -> i32 {
        let px = |v: f32| (v * self.scale).round() as i32;
        if self.view != ViewMode::Rich {
            return self.row_h();
        }
        if let Some((tw, th)) = row.thumb_dims {
            #[allow(clippy::cast_possible_wrap)]
            let (ow, oh) = row
                .img_dims
                .map_or((tw.max(1) as i32, th.max(1) as i32), |(a, b)| {
                    (a as i32, b as i32)
                });
            let (_, dh) = self.rich_fit(ow, oh, content_w);
            dh + px(12.0)
        } else {
            let n = row.rich.as_ref().map_or_else(
                || {
                    row.plain
                        .as_deref()
                        .unwrap_or(row.label.as_str())
                        .lines()
                        .take(5)
                        .count()
                },
                |r| r.len().min(5),
            );
            #[allow(clippy::cast_possible_wrap)]
            let n = n.max(1) as i32;
            px(12.0) + px(22.0) * n
        }
    }

    fn rebuild_offsets(&mut self, w: i32) {
        let cw = self.content_w_of(w);
        let mut offs = Vec::with_capacity(self.rows.len() + 1);
        offs.push(0i32);
        let mut acc = 0i32;
        for row in &self.rows {
            acc += self.row_height_of(row, cw);
            offs.push(acc);
        }
        self.row_offs = offs;
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
        let total = *self.row_offs.last().unwrap_or(&0);
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

    /// 스크롤바 페이드 틱 + ★ 행 hover 의도 수행·페이드(09-04). 반환 = 아직 움직이는 중(셸이 16ms 박동).
    pub(crate) fn tick_ui(&mut self, now_ms: u64) -> bool {
        if let Some(r) = self.row_intent.take_due(now_ms) {
            if self.row_fade.current() != Some(r) {
                self.row_fade.set(Some(r));
                self.redraw();
            }
        }
        if self.bars.tick(now_ms) | self.row_fade.tick(now_ms) {
            self.redraw();
        }
        self.row_fade.is_animating() || self.row_intent.is_waiting(now_ms)
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
        self.scroll = 0;
        self.marked.clear();
        self.was_focused = false;
        self.opened_at = std::time::Instant::now();
        self.refresh(hist);
        // ★ 열 때 선택 = 최신 항목(핀 구획 뒤에 있어도 · 09-02).
        self.sel = self
            .rows
            .iter()
            .position(|r| r.hist_index == 0)
            .unwrap_or(0);
        let mut attrs = crate::settings_win::win_name(crate::icon::with_icon(
            Window::default_attributes()
                .with_title("Nexa Clip")
                // ★ 타이틀바 표시(09-02 사용자) — 리사이즈도 OS 테두리가 맡는다.
                .with_resizable(true)
                .with_min_inner_size(LogicalSize::new(260.0, 200.0))
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_inner_size(LogicalSize::new(POPUP_W, POPUP_H)),
        ));
        // ★ 마지막 크기 복원(물리 px · 09-02) — 기본 크기보다 우선.
        if let Some((pw, ph)) = self.pref_size {
            attrs = attrs.with_inner_size(winit::dpi::PhysicalSize::new(pw.max(200), ph.max(160)));
        }
        if let Some((x, y)) = at {
            // 커서 위치(DR-24 기본) — 물리 좌표 그대로(커서가 곧 물리 좌표다).
            // ★ 화면 경계 클램프(08-31 사용자 실기 "우측 하단이면 팝업이 잘린다") —
            //   커서가 든 모니터 안에 **전체가 들어가도록** 좌상단을 되민다.
            let (x, y) = clamp_to_monitor(el, x, y, self.pref_size);
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
        // ★ 뷰 캐시 비움(09-04 · 30 §2 V) — 다음 open이 다시 만든다.
        self.rows = Vec::new();
    }

    /// 이력 → 필터 통과 행 재구성(검색어 변경·이력 변경 시).
    pub(crate) fn refresh(&mut self, hist: &History) {
        // ★ 세대 가드(28 §hover) — 행 인덱스가 바뀌니 낡은 hover 의도·페이드는 버린다.
        self.row_intent.clear();
        self.row_fade.set(None);
        // ★ 검색 = 라벨 + 본문 전체 · `find.mode`(메인과 동일 · 09-04).
        let matcher =
            nclip_core::search::Matcher::new(&self.search.display_text(), self.search_mode);
        self.rows.clear();
        let mut normal = Vec::new();
        let mut i = 0usize;
        while let Some(item) = hist.get(i) {
            let hit = matcher.is_empty()
                || match self
                    .search_idx
                    .as_ref()
                    .and_then(|ix| ix.borrow().get(&item.id).cloned())
                {
                    Some(t) => matcher.matches_lower(&t),
                    None => matcher.matches(&item.label),
                };
            if hit {
                let plain = crate::main_win::plain_of(&item.reps)
                    .or_else(|| nclip_core::capture::svg_text(&item.reps));
                let row = Row {
                    hist_index: i,
                    id: item.id,
                    pinned: item.pinned,
                    kind: item.kind,
                    label: item.label.clone(),
                    copies: item.copies,
                    remote: item
                        .source_app
                        .as_deref()
                        .is_some_and(|s| s.starts_with(crate::dedup::REMOTE_MARK)),
                    key: crate::dedup::content_key_of(item, plain.as_deref()),
                    origin: item.source_app.clone(),
                    thumb_dims: item.thumb_dims(),
                    plain,
                    img_dims: crate::main_win::display_dims(&item.reps)
                        .or_else(|| crate::main_win::parse_dims(&item.label)),
                    rich: nclip_core::richtext::html_runs_of(&item.reps, 6),
                };
                // ★ 핀 먼저(각 구획 최신순) — 메인창의 정렬 계약과 동일(09-02).
                if row.pinned {
                    self.rows.push(row);
                } else {
                    normal.push(row);
                }
            }
            i += 1;
        }
        self.rows.extend(normal);
        // ★ 병합(09-04 사용자 "메인창의 설정대로") — 같은 내용 한 행(로컬 우선 · 복사 수 합).
        if self.dedup {
            let entries: Vec<crate::dedup::Entry> = self
                .rows
                .iter()
                .map(|r| crate::dedup::Entry {
                    key: r.key,
                    remote: r.remote,
                    origin: r.origin.clone(),
                    copies: r.copies,
                })
                .collect();
            let kept = crate::dedup::merge(&entries);
            let mut rows: Vec<Option<Row>> = std::mem::take(&mut self.rows)
                .into_iter()
                .map(Some)
                .collect();
            self.rows = kept
                .into_iter()
                .filter_map(|k| {
                    let mut r = rows.get_mut(k.keep)?.take()?;
                    r.copies = k.copies;
                    r.remote = k.remote;
                    Some(r)
                })
                .collect();
        }
        if self.sel >= self.rows.len() {
            self.sel = self.rows.len().saturating_sub(1);
        }
        self.scroll = self
            .scroll
            .min(self.row_offs.get(self.sel).copied().unwrap_or(0));
    }

    /// ★ 이력이 바뀌었다(새 복사·승격) — **커서를 맨 위로**(방금 것이 첫 줄이고
    /// 선택도 그걸 가리킨다 — 08-28 사용자 요청) + 다시 그린다.
    pub(crate) fn on_history_changed(&mut self, hist: &History) {
        self.scroll = 0;
        self.refresh(hist);
        // ★ 선택 = 방금 복사한 항목(이력 맨 앞) — 핀 구획이 앞에 와도 계약 유지(08-28·09-02).
        self.sel = self
            .rows
            .iter()
            .position(|r| r.hist_index == 0)
            .unwrap_or(0);
        self.ensure_visible();
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
            WindowEvent::Resized(sz) => {
                // ★ 마지막 크기 기억(09-02) — 닫을 때 셸이 ui.popup_w/h로 저장.
                if sz.width > 0 && sz.height > 0 {
                    self.last_size = Some((sz.width, sz.height));
                }
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if self.feed_bars(event) {
                    return PopupAction::None;
                }
                let (x, y) = self.cursor;
                // ★ 행 hover = 의도만 등록(09-04 · 28 §hover) — 벗어나면 즉시 끔.
                match self.row_at(x, y) {
                    Some(r) => self.row_intent.set(r, crate::main_win::now_ms()),
                    None => {
                        self.row_intent.clear();
                        if self.row_fade.current().is_some() {
                            self.row_fade.set(None);
                            self.redraw();
                        }
                    }
                }
                if self.feed_search(&CtlEvent::MouseMove { x, y }) {
                    self.sel = 0;
                    self.scroll = 0;
                    self.refresh(hist);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.row_intent.clear();
                if self.row_fade.current().is_some() {
                    self.row_fade.set(None);
                    self.redraw();
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
                        // ★ Ctrl+클릭 = 스택 토글(09-03 ③ — 즉시 붙여넣기 대신 선택만).
                        if self.ctrl {
                            self.sel = vi;
                            self.toggle_mark(vi);
                            return PopupAction::None;
                        }
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
                // ★ 스크롤 안정 대기(28 §hover) — 멈춘 뒤 커서 아래 행만.
                let now = crate::main_win::now_ms();
                self.row_intent.settle(now);
                if let Some(r) = self.row_at(self.cursor.0, self.cursor.1) {
                    self.row_intent.set(r, now);
                }
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
                        self.ensure_visible();
                        self.redraw();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.sel + 1 < self.rows.len() {
                            self.sel += 1;
                        }
                        self.ensure_visible();
                        self.redraw();
                    }
                    Key::Named(NamedKey::Enter) => {
                        // ★ 스택이 쌓여 있으면 Enter = 순차 붙여넣기(09-03 ③).
                        if !self.marked.is_empty() {
                            return PopupAction::PickStack(std::mem::take(&mut self.marked));
                        }
                        if let Some(row) = self.rows.get(self.sel) {
                            return PopupAction::Pick {
                                index: row.hist_index,
                                as_: self.paste_mode(),
                            };
                        }
                        return PopupAction::Close;
                    }
                    // ★ Ctrl+Space = 현재 행 스택 토글(Space 단독은 검색어 몫 · 09-03 ③).
                    Key::Named(NamedKey::Space) if self.ctrl => {
                        self.toggle_mark(self.sel);
                        return PopupAction::None;
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
        let Some(win) = self.window.clone() else {
            return;
        };
        let size = win.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        // ★ 선택 가시화 스크롤·오프셋은 **surface 빌림 전에** 계산한다(필드 분리 빌림).
        let (iw, ih) = (size.width as i32, size.height as i32);
        let sc = self.scale;
        let px = move |v: f32| (v * sc).round() as i32;
        let list_h = ((ih - px(24.0)) - px(38.0)).max(1);
        self.rebuild_offsets(iw);
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        {
            // ★ 상한 클램프만 — 매 프레임 선택 추종은 휠이 옮긴 위치를 되돌려
            //   "선택 아래로 스크롤 불가"가 된다(09-02 실기 — 메인 4차와 같은 결함).
            //   추종은 키 이동 몷(ensure_visible).
            let total = *self.row_offs.last().unwrap_or(&0);
            self.scroll = self.scroll.clamp(0, (total - list_h).max(0));
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        {
            let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
            // ★ 배율은 레이아웃과 같은 값(08-27 macOS 회귀의 교훈).
            let mut fonts = nclip_ctl::raster::FontSet::single(&self.font);
            fonts.mono = self.font_mono.as_ref();
            let mut dc = RasterCtx::with_font_set(&mut gfx, fonts, self.scale)
                .with_caret_on(self.caret_phase);
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
                self.view,
                &self.row_offs,
                &self.marked,
                self.thumbs.as_ref(),
                &self.row_fade,
            );
            let total = *self.row_offs.last().unwrap_or(&0);
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
/// 행의 섬네일 — 캐시에 있으면 그것, 없으면 요청만 남기고 None(메인과 같은 규칙).
fn thumb_for(
    thumbs: Option<&crate::thumbs::Thumbs>,
    row: &Row,
) -> Option<std::rc::Rc<nclip_ctl::theme::IconImage>> {
    row.thumb_dims?;
    let mut c = thumbs?.borrow_mut();
    c.get(row.id).or_else(|| {
        c.want(row.id);
        None
    })
}

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
    view: ViewMode,
    offs: &[i32],
    marked: &[u64],
    thumbs: Option<&crate::thumbs::Thumbs>,
    row_fade: &nclip_ctl::tokens::HoverFade,
) {
    let px = |v: f32| (v * s).round() as i32;
    let full = Rect::new(0, 0, w, h);
    dc.select_font(FontSlot::Base, false);

    let header_h = px(38.0);
    let footer_h = px(24.0);
    let pad = px(10.0);
    let row_h = match view {
        ViewMode::Rich => px(76.0),
        ViewMode::Compact => px(30.0),
        ViewMode::Plain => px(22.0),
    }
    .max(1);

    dc.fill_rect(full, th.window_bg);

    // ── 헤더: 검색 필드(정식 TextBox — 캐럿·선택·×· 09-02) ──
    dc.fill_rect(Rect::new(0, 0, w, header_h), th.chrome_bg);
    search.paint(dc, &th);
    dc.fill_rect(Rect::new(0, header_h - 1, w, 1), th.border);

    // ── 목록 — ★ 보기 3모드(`ui.popup_view` · 09-02) · 가변 행은 누적합.
    let list_top = header_h;
    let list_bot = h - footer_h;
    let first = offs.partition_point(|&o| o <= scroll).saturating_sub(1);
    if rows.is_empty() {
        let lang = current_lang();
        let msg = if search.display_text().is_empty() {
            tr(lang, Msg::PopupNoItems)
        } else {
            tr(lang, Msg::MainNoMatch)
        };
        dc.text(pad, list_top + px(12.0), full, msg, th.text_dim);
    }
    let mut pin_divider_done = false;
    for (vi, row) in rows.iter().enumerate().skip(first) {
        // ★ 섬네일은 캐시에서(09-04 · 30 §4) — 없으면 요청만.
        let thumb = thumb_for(thumbs, row);
        let y = list_top - scroll + offs.get(vi).copied().unwrap_or(0);
        if y >= list_bot {
            break;
        }
        let rh = offs
            .get(vi + 1)
            .map_or(row_h, |e| e - offs.get(vi).copied().unwrap_or(0));
        let cy0 = y.max(list_top);
        let clip = Rect::new(0, cy0, w, ((y + rh).min(list_bot) - cy0).max(0));
        if vi == sel {
            dc.fill_rect(clip, th.sel_bg);
        } else if vi % 2 == 1 {
            dc.fill_rect(clip, th.panel_bg_alt);
        }
        // ★ hover 상태 레이어(09-04 사용자 — 메인과 동일): 선택 행 제외 · 본문색 6% × 페이드.
        let g = row_fade.value(vi);
        if vi != sel && g > 0.0 {
            dc.fill_rect_alpha(clip, th.text, 0.06 * g);
        }
        // 핀 구획 경계 — 첫 비고정 행 위 한 줄(메인과 동일 · 부분 행이면 생략).
        if !pin_divider_done && !row.pinned && vi > 0 {
            if y >= list_top {
                dc.fill_rect(Rect::new(0, y, w, 1), th.accent);
            }
            pin_divider_done = true;
        }
        // 핀 점 — 좌측 거터(행 clip 안일 때만).
        if row.pinned {
            let dot_y = y + px(11.0);
            if dot_y >= clip.y && dot_y + px(6.0) <= clip.y + clip.h {
                dc.fill_round_rect(
                    Rect::new(px(3.0), dot_y, px(5.0), px(5.0)),
                    px(2.5),
                    th.accent,
                );
            }
        }
        // ★ 수신 점(09-04 — 메인창과 동일): 핀 점 옆(점 5 + 간격 2) · 상태줄 릴레이 녹색.
        if row.remote {
            let dot_y = y + px(11.0);
            let dot_x = if row.pinned { px(10.0) } else { px(3.0) };
            if dot_y >= clip.y && dot_y + px(6.0) <= clip.y + clip.h {
                dc.fill_round_rect(
                    Rect::new(dot_x, dot_y, px(5.0), px(5.0)),
                    px(2.5),
                    nclip_ctl::theme::Color::from_rgb(46, 204, 64),
                );
            }
        }
        // ★ 스택 표시(09-03 ③) — 좌측 accent 바 + 선택 순번.
        if let Some(pos) = marked.iter().position(|&m| m == row.id) {
            dc.fill_rect(Rect::new(0, clip.y, px(3.0).max(2), clip.h), th.accent);
            dc.select_font(FontSlot::Status, false);
            let tag = format!("{}", pos + 1);
            let tw = dc.text_width(&tag);
            dc.text(w - pad - tw, y + rh - px(16.0), clip, &tag, th.accent);
            dc.select_font(FontSlot::Base, false);
        }
        // 우측 ×n — 모든 모드 공통.
        let mut right = w - pad;
        if row.copies > 1 {
            let tag = format!("×{}", row.copies);
            let tw = dc.text_width(&tag);
            right -= tw;
            dc.text(right, y + px(6.0), clip, &tag, th.text_dim);
            right -= px(8.0);
        }
        if view == ViewMode::Rich {
            // ── Rich = CopyQ 화법(메인과 동일 · 09-02): 거터 번호 + 내용 그 자체.
            dc.select_font(FontSlot::Status, false);
            let no = format!("{}", vi + 1);
            dc.text(pad, y + px(6.0), clip, &no, th.text_dim);
            dc.select_font(FontSlot::Base, false);
            let cx0 = pad + px(30.0);
            let content_clip = Rect::new(cx0, clip.y, (right - cx0).max(0), clip.h);
            if let Some(img) = &thumb {
                #[allow(clippy::cast_possible_wrap)]
                let (ow, oh) = row
                    .img_dims
                    .map_or((img.w.max(1) as i32, img.h.max(1) as i32), |(a, b)| {
                        (a as i32, b as i32)
                    });
                // rich_fit와 같은 규약(64% · 폭·최대 200px) — 자유 함수라 여기 다시 편다.
                let max_h = px(200.0);
                let (ow, oh) = ((ow * 16 / 25).max(1), (oh * 16 / 25).max(1));
                let content_w = (w - pad * 2 - px(30.0)).max(40);
                let mut dw = ow.min(content_w);
                let mut dh = (oh * dw / ow).max(1);
                if dh > max_h {
                    dw = (dw * max_h / dh).max(1);
                    dh = max_h;
                }
                let dst = Rect::new(cx0, y + px(6.0), dw, dh);
                dc.image_scaled(dst, img, content_clip);
            } else if let Some((tw, tht)) = row.thumb_dims {
                // ★ 디코드 대기 자리표시(09-04 · 30 §4) — 메인과 같은 규약.
                #[allow(clippy::cast_possible_wrap)]
                let (ow, oh) = row
                    .img_dims
                    .map_or((tw.max(1) as i32, tht.max(1) as i32), |(a, b)| {
                        (a as i32, b as i32)
                    });
                let max_h = px(200.0);
                let (ow, oh) = ((ow * 16 / 25).max(1), (oh * 16 / 25).max(1));
                let content_w = (w - pad * 2 - px(30.0)).max(40);
                let mut dw = ow.min(content_w);
                let mut dh = (oh * dw / ow).max(1);
                if dh > max_h {
                    dw = (dw * max_h / dh).max(1);
                    dh = max_h;
                }
                dc.fill_round_rect(
                    crate::main_win::clip_to(Rect::new(cx0, y + px(6.0), dw, dh), content_clip),
                    px(4.0),
                    th.panel_bg_alt,
                );
            } else if let Some(rich) = &row.rich {
                // ★ T-18d 1단 — 메인과 동일(색·굵기 · 탭 스톱 열맞춤).
                let tab_w = dc.text_width("    ").max(8);
                let em = dc.text_width("한").max(8);
                for (k, line) in rich.iter().take(5).enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let ly = y + px(6.0 + 22.0 * k as f32);
                    let mut xoff = 0i32;
                    for run in line {
                        dc.select_font_sized(
                            if run.mono {
                                FontSlot::Mono
                            } else {
                                FontSlot::Base
                            },
                            run.bold,
                            nclip_core::richtext::size_delta(em, run.scale),
                        );
                        xoff += nclip_core::richtext::em_px(em, run.indent);
                        let col = run.color.map_or(th.text, |c| {
                            nclip_ctl::theme::Color::from_rgb(c[0], c[1], c[2])
                        });
                        for (ti, seg) in run.text.split('\t').enumerate() {
                            if ti > 0 {
                                xoff = (xoff / tab_w + 1) * tab_w;
                            }
                            if !seg.is_empty() {
                                let sw = dc.text_width(seg);
                                if let Some(b) = run.bg {
                                    dc.fill_rect(
                                        crate::main_win::clip_to(
                                            Rect::new(cx0 + xoff, ly, sw, px(22.0)),
                                            content_clip,
                                        ),
                                        nclip_ctl::theme::Color::from_rgb(b[0], b[1], b[2]),
                                    );
                                }
                                dc.text(cx0 + xoff, ly, content_clip, seg, col);
                                xoff += sw;
                            }
                        }
                    }
                }
                dc.select_font(FontSlot::Base, false);
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
        let text_y = if view == ViewMode::Plain {
            y + px(3.0)
        } else {
            y + px(7.0)
        };
        let lx = if view == ViewMode::Plain {
            pad
        } else {
            // ★ 이미지 썸네일(설정 켜짐 · 디코드 성공 시) — 비율 유지로 24px 상자에.
            if let Some(img) = &thumb {
                let box_side = px(24.0);
                let (iw2, ih2) = (img.w.max(1) as i32, img.h.max(1) as i32);
                let (dw, dh) = if iw2 >= ih2 {
                    (box_side, (box_side * ih2 / iw2).max(1))
                } else {
                    ((box_side * iw2 / ih2).max(1), box_side)
                };
                let dst = Rect::new(pad + (box_side - dw) / 2, y + (rh - dh) / 2, dw, dh);
                dc.image_scaled(dst, img, clip);
            } else {
                dc.text(pad, text_y, clip, kind_glyph(row.kind), th.accent);
            }
            pad + px(30.0)
        };
        let label_clip = Rect::new(lx, clip.y, (right - lx).max(0), clip.h);
        dc.text(lx, text_y, label_clip, &row.label, th.text);
        // 이미지·개체 식별 보조 — 라벨 뒤 본문 첫 줄(메인 Ctrl+2와 동일 · 09-02).
        if matches!(row.kind, ClipKind::Image | ClipKind::Object) {
            if let Some(fst) = row.plain.as_deref().and_then(|pl| pl.lines().next()) {
                if !fst.trim().is_empty() {
                    let snippet: String = fst.chars().take(120).collect();
                    let sx = lx + dc.text_width(&row.label) + px(8.0);
                    dc.text(sx, text_y, label_clip, &snippet, th.text_dim);
                }
            }
        }
    }

    // ── 푸터: 키 힌트 1줄 ──
    let fy = h - footer_h;
    dc.fill_rect(Rect::new(0, fy, w, footer_h), th.chrome_bg);
    dc.fill_rect(Rect::new(0, fy, w, 1), th.border);
    // ★ 힌트: 스택이 쌓이면 스택 힌트(09-03 ③) · 아니면 항목 종류(DR-35).
    let lang = current_lang();
    let stack_hint: String;
    let hint: &str = if marked.is_empty() {
        match rows.get(sel).map(|r| r.kind) {
            Some(ClipKind::Files) => tr(lang, Msg::HintFiles),
            Some(ClipKind::RichText) => tr(lang, Msg::HintRich),
            Some(ClipKind::Image | ClipKind::Object) => tr(lang, Msg::HintImage),
            _ => tr(lang, Msg::HintDefault),
        }
    } else {
        stack_hint = tr(lang, Msg::HintStack).replacen("{}", &marked.len().to_string(), 1);
        &stack_hint
    };
    dc.text(pad, fy + px(5.0), full, hint, th.text_dim);

    // 검색 우클릭 편집 메뉴 — 맨 위 레이어(z = 그리는 순서).
    search.paint_popup(dc, &th);
}
