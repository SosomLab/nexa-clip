//! ★ **로그 화면**(S9 · T-22b · 11-08 사용자 확정 "설정 창 안 뷰") — 설정 ▸ 고급 ▸ \[로그 보기…\].
//!
//! [DR-31](../../../docs/10-decision-record.md): 실패 통보는 **상태 전이 + 로그**(모달 없음) — 여기가 "원인 + **조치**"를
//! 사용자에게 보여 주는 기본 채널이다. 호스트([`nclip_core::diag::DiagLog`]를 쥔 셸)가 스냅숏([`LogEntry`] 목록 + 잘린 개수)을
//! 넣고, 위젯은 목록·선택 상세·심각도 필터·\[복사\]\[지우기\]\[닫기\]만 그린다 — 라이선스 화면([`crate::license_win`])과
//! 같은 **winit 없는 위젯** 꼴(행동은 [`LogWidget::take_action`] 1회성).
//!
//! 위 = 필터 3버튼(전체 · 주의 이상 · 실패) + 잘림 안내 · 가운데 = 목록(시각 · 심각도 점 · 무엇) · 아래 = 선택 상세(대상 · 원인 ·
//! 조치 — 접힌 글 워드랩) + 버튼 행. 항목 **내용**은 로그에 없다(L-4) — 식별·사유·조치만.

use nclip_core::diag::Level;
use nclip_core::{t, tf, Msg};
use nexa_ctl::controls::{Button, ButtonTone, Control as _};
use nexa_ctl::draw::{ellipsize_middle, DrawCtx, FontSlot};
use nexa_ctl::event::{InputEvent, Key, WHEEL_DELTA};
use nexa_ctl::geom::{Point, Rect};
use nexa_ctl::theme::{Color, Theme};
use nexa_ctl::widget::{Invalidations, Widget};

/// 한 줄(호스트가 `Record`에서 만든다 — 시각은 로컬 시간 문자열로 미리 풀어 준다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// 로컬 시각 `HH:MM:SS`.
    pub time: String,
    pub level: Level,
    pub what: String,
    pub subject: Option<String>,
    pub cause: String,
    pub action: String,
}

/// 위젯이 호스트에게 내는 행동.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogAction {
    /// 보이는(필터 통과) 줄 전부를 글로 — 호스트가 클립보드에 쓴다.
    Copy(String),
    /// 전부 지우기(호스트가 `DiagLog::clear` 뒤 [`LogWidget::set_entries`]).
    Clear,
    /// 설정 화면으로.
    Close,
}

const PAD: i32 = 16;
const BTN_H: i32 = 30;
const ROW_H: i32 = 24;
const GAP: i32 = 8;
const DETAIL_LINES: i32 = 5;

/// 로그 화면 위젯.
#[derive(Debug)]
pub struct LogWidget {
    bounds: Rect,
    scale: f32,
    entries: Vec<LogEntry>,
    dropped: u64,
    /// 심각도 하한(필터).
    min: Level,
    /// 선택(필터 통과 목록의 인덱스).
    sel: Option<usize>,
    /// 목록 스크롤(행 단위).
    first: usize,
    f_all: Button,
    f_warn: Button,
    f_err: Button,
    btn_copy: Button,
    btn_clear: Button,
    btn_close: Button,
    action: Option<LogAction>,
    /// 마지막 결과 한 줄(복사됨 등).
    note: Option<String>,
}

impl Default for LogWidget {
    fn default() -> Self {
        Self::new(Vec::new(), 0)
    }
}

impl LogWidget {
    /// 스냅숏으로 만든다(최신이 **위**에 오게 호스트가 뒤집어 준다 — `DiagLog::iter`는 오래된 것부터).
    #[must_use]
    pub fn new(entries: Vec<LogEntry>, dropped: u64) -> Self {
        let mut w = Self {
            bounds: Rect::default(),
            scale: 1.0,
            entries,
            dropped,
            min: Level::Info,
            sel: None,
            first: 0,
            f_all: Button::new(t(Msg::LogFilterAll)),
            f_warn: Button::new(t(Msg::LogFilterWarn)),
            f_err: Button::new(t(Msg::LogFilterError)),
            btn_copy: Button::new(t(Msg::LogBtnCopy)),
            btn_clear: Button::new(t(Msg::LogBtnClear)),
            btn_close: Button::new(t(Msg::LogBtnClose)),
            action: None,
            note: None,
        };
        w.sync_filter_buttons();
        w.sel = (!w.visible().is_empty()).then_some(0);
        w
    }

    /// 스냅숏 교체(지우기 뒤 · 새 줄이 쌓인 뒤 다시 열 때).
    pub fn set_entries(&mut self, entries: Vec<LogEntry>, dropped: u64, inv: &mut Invalidations) {
        self.entries = entries;
        self.dropped = dropped;
        self.first = 0;
        self.sel = (!self.visible().is_empty()).then_some(0);
        inv.push(self.bounds);
    }

    /// 결과 한 줄(복사됨 등).
    pub fn set_note(&mut self, text: impl Into<String>, inv: &mut Invalidations) {
        self.note = Some(text.into());
        inv.push(self.bounds);
    }

    /// 결과 한 줄 지우기([지우기] 뒤 · 11-08).
    pub fn clear_note(&mut self, inv: &mut Invalidations) {
        if self.note.take().is_some() {
            inv.push(self.bounds);
        }
    }

    /// 1회성 행동.
    pub fn take_action(&mut self) -> Option<LogAction> {
        self.action.take()
    }

    pub fn set_scale(&mut self, scale: f32, inv: &mut Invalidations) {
        self.scale = scale.max(0.5);
        for b in self.buttons_mut() {
            b.set_scale(scale.max(0.5));
        }
        self.relayout(inv);
    }

    /// 지금 필터(시험·덤프용).
    #[must_use]
    pub fn min_level(&self) -> Level {
        self.min
    }

    /// 필터 통과 줄 수.
    #[must_use]
    pub fn visible_len(&self) -> usize {
        self.visible().len()
    }

    /// 보이는 줄 전부를 복사용 글로(한 줄 = 시각 · 심각도 · 무엇 · 대상 · 원인 · 조치).
    #[must_use]
    pub fn render_visible(&self) -> String {
        let mut out = String::new();
        for e in self.visible() {
            out.push_str(&format!("[{}] [{:?}] {}\n", e.time, e.level, e.what));
            if let Some(s) = &e.subject {
                out.push_str(&format!("  {}: {s}\n", t(Msg::LogColSubject)));
            }
            out.push_str(&format!("  {}: {}\n", t(Msg::LogColCause), e.cause));
            out.push_str(&format!("  {}: {}\n", t(Msg::LogColAction), e.action));
        }
        out
    }

    fn visible(&self) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.level >= self.min)
            .collect()
    }

    fn buttons_mut(&mut self) -> [&mut Button; 6] {
        [
            &mut self.f_all,
            &mut self.f_warn,
            &mut self.f_err,
            &mut self.btn_copy,
            &mut self.btn_clear,
            &mut self.btn_close,
        ]
    }

    fn sync_filter_buttons(&mut self) {
        let min = self.min;
        for (b, lv) in [
            (&mut self.f_all, Level::Info),
            (&mut self.f_warn, Level::Warn),
            (&mut self.f_err, Level::Error),
        ] {
            b.set_tone(if lv == min {
                ButtonTone::Accent
            } else {
                ButtonTone::Default
            });
        }
    }

    fn s(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    /// 목록 영역(필터 줄 아래 ~ 상세 위).
    fn list_rect(&self) -> Rect {
        let b = self.bounds;
        let pad = self.s(PAD);
        let top = b.y + pad + self.s(BTN_H) + self.s(GAP) + self.s(ROW_H); // 필터 줄 + 잘림/개수 안내 줄
        let bottom = self.detail_rect().y - self.s(GAP);
        Rect::new(
            b.x + pad,
            top,
            b.w - pad * 2,
            (bottom - top).max(self.s(ROW_H)),
        )
    }

    /// 상세 영역(버튼 행 위 고정 높이).
    fn detail_rect(&self) -> Rect {
        let b = self.bounds;
        let pad = self.s(PAD);
        let h = self.s(ROW_H) * DETAIL_LINES + self.s(GAP);
        let y = b.bottom() - pad - self.s(BTN_H) - self.s(GAP) - h;
        Rect::new(b.x + pad, y, b.w - pad * 2, h)
    }

    fn rows_fit(&self) -> usize {
        usize::try_from(self.list_rect().h / self.s(ROW_H).max(1))
            .unwrap_or(0)
            .max(1)
    }

    fn relayout(&mut self, inv: &mut Invalidations) {
        let b = self.bounds;
        let pad = self.s(PAD);
        let bh = self.s(BTN_H);
        let gap = self.s(GAP);
        // 필터 3버튼 — 위 왼쪽.
        let mut x = b.x + pad;
        for b_ in [&mut self.f_all, &mut self.f_warn, &mut self.f_err] {
            let w = (label_w(b_.label()) as f32 * self.scale).round() as i32;
            b_.set_bounds(Rect::new(x, b.y + pad, w, bh), inv);
            x += w + gap;
        }
        // 아래 버튼 행 — 왼쪽부터 [복사][지우기] · 오른쇽 끝 [닫기].
        let by = b.bottom() - pad - bh;
        let mut bx = b.x + pad;
        for b_ in [&mut self.btn_copy, &mut self.btn_clear] {
            let w = (label_w(b_.label()) as f32 * self.scale).round() as i32;
            b_.set_bounds(Rect::new(bx, by, w, bh), inv);
            bx += w + gap;
        }
        let cw = (label_w(self.btn_close.label()) as f32 * self.scale).round() as i32;
        self.btn_close
            .set_bounds(Rect::new(b.right() - pad - cw, by, cw, bh), inv);
        self.clamp_scroll();
        inv.push(b);
    }

    fn clamp_scroll(&mut self) {
        let n = self.visible().len();
        let fit = self.rows_fit();
        self.first = self.first.min(n.saturating_sub(fit));
    }

    fn ensure_visible(&mut self) {
        if let Some(i) = self.sel {
            let fit = self.rows_fit();
            if i < self.first {
                self.first = i;
            } else if i >= self.first + fit {
                self.first = i + 1 - fit;
            }
        }
    }

    fn set_filter(&mut self, min: Level, inv: &mut Invalidations) {
        if self.min != min {
            self.min = min;
            self.first = 0;
            self.sel = (!self.visible().is_empty()).then_some(0);
            self.sync_filter_buttons();
            inv.push(self.bounds);
        }
    }

    fn take_clicks(&mut self, inv: &mut Invalidations) {
        if self.f_all.take_clicked() {
            self.set_filter(Level::Info, inv);
        } else if self.f_warn.take_clicked() {
            self.set_filter(Level::Warn, inv);
        } else if self.f_err.take_clicked() {
            self.set_filter(Level::Error, inv);
        } else if self.btn_copy.take_clicked() {
            self.action = Some(LogAction::Copy(self.render_visible()));
        } else if self.btn_clear.take_clicked() {
            self.action = Some(LogAction::Clear);
        } else if self.btn_close.take_clicked() {
            self.action = Some(LogAction::Close);
        }
    }
}

/// 버튼 폭(논리 px) — 글자 수 기반 어림(라이선스 화면과 같은 규칙).
fn label_w(label: &str) -> i32 {
    let units: i32 = label
        .chars()
        .map(|c| if c.is_ascii() { 7 } else { 14 })
        .sum();
    (units + 28).max(72)
}

fn level_color(theme: &Theme, lv: Level) -> Color {
    match lv {
        Level::Info => theme.accent,
        Level::Warn => theme.warn,
        Level::Error => theme.danger,
    }
}

impl Widget for LogWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect, inv: &mut Invalidations) {
        self.bounds = bounds;
        self.relayout(inv);
    }

    fn on_event(&mut self, ev: &InputEvent, inv: &mut Invalidations) {
        match *ev {
            InputEvent::Key {
                key: Key::Escape, ..
            } => {
                self.action = Some(LogAction::Close);
                return;
            }
            InputEvent::Key { key: Key::Up, .. } => {
                if let Some(i) = self.sel {
                    self.sel = Some(i.saturating_sub(1));
                    self.ensure_visible();
                    inv.push(self.bounds);
                }
                return;
            }
            InputEvent::Key { key: Key::Down, .. } => {
                let n = self.visible().len();
                if let Some(i) = self.sel {
                    if i + 1 < n {
                        self.sel = Some(i + 1);
                        self.ensure_visible();
                        inv.push(self.bounds);
                    }
                }
                return;
            }
            InputEvent::Wheel { delta } => {
                let notches = -(delta / WHEEL_DELTA.max(1));
                let n = self.visible().len();
                let fit = self.rows_fit();
                let max = n.saturating_sub(fit);
                let cur = i64::try_from(self.first).unwrap_or(0);
                let next = (cur + i64::from(notches) * 3).clamp(0, i64::try_from(max).unwrap_or(0));
                self.first = usize::try_from(next).unwrap_or(0);
                inv.push(self.bounds);
                return;
            }
            InputEvent::MouseDown { x, y, .. } => {
                let p = Point { x, y };
                let lr = self.list_rect();
                if lr.contains(p) {
                    let row = usize::try_from((y - lr.y) / self.s(ROW_H).max(1)).unwrap_or(0);
                    let idx = self.first + row;
                    if idx < self.visible().len() {
                        self.sel = Some(idx);
                        inv.push(self.bounds);
                    }
                }
                for b in self.buttons_mut() {
                    if b.bounds().contains(p) {
                        b.on_event(ev, inv);
                    }
                }
            }
            InputEvent::MouseUp { .. } | InputEvent::MouseMove { .. } => {
                for b in self.buttons_mut() {
                    b.on_event(ev, inv);
                }
            }
            _ => {}
        }
        self.take_clicks(inv);
    }

    fn paint(&self, ctx: &mut dyn DrawCtx, theme: &Theme) {
        let b = self.bounds;
        ctx.fill_rect(b, theme.panel_bg);
        ctx.select_font(FontSlot::Base, false);
        let th = ctx.text_height();
        let pad = self.s(PAD);
        let row_h = self.s(ROW_H);
        for btn in [
            &self.f_all,
            &self.f_warn,
            &self.f_err,
            &self.btn_copy,
            &self.btn_clear,
            &self.btn_close,
        ] {
            btn.paint(ctx, theme);
        }
        // 안내 줄(필터 아래) — 개수 · 잘림.
        let vis = self.visible();
        let info_y = b.y + pad + self.s(BTN_H) + self.s(GAP);
        let mut info = tf(
            Msg::LogCount,
            &[&vis.len().to_string(), &self.entries.len().to_string()],
        );
        if self.dropped > 0 {
            info.push_str(" · ");
            info.push_str(&tf(Msg::LogDropped, &[&self.dropped.to_string()]));
        }
        ctx.text(
            b.x + pad,
            info_y + (row_h - th) / 2,
            b,
            &info,
            theme.text_dim,
        );
        // 목록.
        let lr = self.list_rect();
        ctx.fill_rect(lr, theme.field_bg);
        if vis.is_empty() {
            ctx.text(
                lr.x + self.s(8),
                lr.y + (row_h - th) / 2,
                lr,
                t(Msg::LogEmpty),
                theme.text_dim,
            );
        }
        let fit = self.rows_fit();
        let dot = self.s(8);
        for (k, e) in vis.iter().enumerate().skip(self.first).take(fit) {
            let y = lr.y + i32::try_from(k - self.first).unwrap_or(0) * row_h;
            let r = Rect::new(lr.x, y, lr.w, row_h);
            if self.sel == Some(k) {
                ctx.fill_rect(r, theme.sel_bg);
            }
            let c = level_color(theme, e.level);
            ctx.fill_rect(
                Rect::new(r.x + self.s(8), y + (row_h - dot) / 2, dot, dot),
                c,
            );
            let tx = r.x + self.s(8) + dot + self.s(8);
            let time_w = ctx.text_width(&e.time) + self.s(10);
            ctx.text(tx, y + (row_h - th) / 2, r, &e.time, theme.text_dim);
            let what = ellipsize_middle(ctx, &e.what, r.w - (tx - r.x) - time_w - self.s(8));
            ctx.text(tx + time_w, y + (row_h - th) / 2, r, &what, theme.text);
        }
        // 상세.
        let dr = self.detail_rect();
        ctx.fill_rect(dr, theme.panel_bg_alt);
        if let Some(e) = self.sel.and_then(|i| vis.get(i)) {
            let mut y = dr.y + self.s(4);
            let label_w = [Msg::LogColSubject, Msg::LogColCause, Msg::LogColAction]
                .iter()
                .map(|m| ctx.text_width(t(*m)))
                .max()
                .unwrap_or(0)
                + self.s(12);
            let lines: [(Msg, &str); 3] = [
                (Msg::LogColSubject, e.subject.as_deref().unwrap_or("-")),
                (Msg::LogColCause, &e.cause),
                (Msg::LogColAction, &e.action),
            ];
            for (m, v) in lines {
                ctx.text(dr.x + self.s(8), y, dr, t(m), theme.text_dim);
                let vr = Rect::new(
                    dr.x + self.s(8) + label_w,
                    y,
                    dr.w - self.s(16) - label_w,
                    row_h,
                );
                // 조치는 눈에 띄게(DR-31 — "조치까지 적는다").
                let fg = if m == Msg::LogColAction {
                    theme.text
                } else {
                    theme.text_dim
                };
                let v = ellipsize_middle(ctx, v, vr.w);
                ctx.text(vr.x, y, vr, &v, fg);
                y += row_h;
            }
        }
        // 결과 한 줄(버튼 행 사이).
        if let Some(n) = &self.note {
            let x = self.btn_clear.bounds().right() + self.s(12);
            let by = self.btn_close.bounds().y;
            let nr = Rect::new(
                x,
                by,
                self.btn_close.bounds().x - x - self.s(8),
                self.s(BTN_H),
            );
            let n = ellipsize_middle(ctx, n, nr.w);
            ctx.text(nr.x, by + (self.s(BTN_H) - th) / 2, nr, &n, theme.ok);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(lv: Level, what: &str) -> LogEntry {
        LogEntry {
            time: "12:00:00".into(),
            level: lv,
            what: what.into(),
            subject: None,
            cause: "c".into(),
            action: "a".into(),
        }
    }

    #[test]
    fn filter_hides_lower_levels_and_render_lists_visible() {
        let _lang = crate::lang_test_lock();
        nclip_core::set_lang(nclip_core::Lang::En);
        let mut w = LogWidget::new(
            vec![
                entry(Level::Error, "e1"),
                entry(Level::Warn, "w1"),
                entry(Level::Info, "i1"),
            ],
            0,
        );
        let mut inv = Invalidations::default();
        w.set_bounds(Rect::new(0, 0, 700, 500), &mut inv);
        assert_eq!(w.visible_len(), 3);
        w.set_filter(Level::Warn, &mut inv);
        assert_eq!(w.visible_len(), 2);
        let txt = w.render_visible();
        assert!(txt.contains("e1") && txt.contains("w1") && !txt.contains("i1"));
        w.set_filter(Level::Error, &mut inv);
        assert_eq!(w.visible_len(), 1);
        assert_eq!(w.sel, Some(0));
    }

    #[test]
    fn escape_closes_and_clear_button_emits() {
        let mut w = LogWidget::new(vec![entry(Level::Info, "x")], 3);
        let mut inv = Invalidations::default();
        w.set_bounds(Rect::new(0, 0, 700, 500), &mut inv);
        w.on_event(
            &InputEvent::Key {
                key: Key::Escape,
                shift: false,
                primary: false,
            },
            &mut inv,
        );
        assert_eq!(w.take_action(), Some(LogAction::Close));
    }
}
