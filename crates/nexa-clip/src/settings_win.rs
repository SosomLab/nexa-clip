//! `settings` — ★ **설정 화면을 실제로 띄운다**(T-12d2).
//!
//! [`nclip_ui::SettingsWidget`](nclip_ui::SettingsWidget)은 `nexa-beep`에서 이식한
//! **프레임워크 2,000줄**이고([13 §2-3](../../../docs/13-ui-reuse-from-beep.md)),
//! 항목은 우리 [`registry()`](nclip_ui::registry)가 준다. 여기서 하는 일은
//! **창·입력을 위젯에 잇는 것**뿐이다 — 화면 코드는 이식본 그대로다.
//!
//! ## 확인되는 것
//!
//! | 항목 | 어떻게 |
//! |---|---|
//! | 이식이 실제로 도는가 | 창이 뜨고 **좌측 카테고리 + 우측 폼**이 그려진다 |
//! | ★ **설정 검색** | 상단에 타이핑 → **레지스트리 단일 원천**이 걸러진다 |
//! | ★ 즉시 적용 | 값을 바꾸면 `take_changes()`가 방출한다(콘솔에 찍는다) |
//! | 우리 레지스트리 | 21항목 · 카테고리 8개가 우리 것으로 나온다 |
//! | ★ **스플리터** | 사이드바 경계에 커서를 두면 **서서히 밝아지고** 좌우 리사이즈 커서 · 드래그로 조절 |
//! | ★ **영속**(T-12c2) | 값을 바꾸고 창을 닫았다 다시 열면 **그대로 있다**([`crate::conf`]) |

use nclip_ctl::draw::DrawCtx;
use nclip_ctl::event::{InputEvent, Key as CtlKey, WHEEL_DELTA};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
use nclip_ctl::widget::{Invalidations, Widget};
use nclip_gfx::{Font, Surface};
use nclip_ui::SettingsWidget;

use crate::conf::Settings;

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use nclip_ctl::draw::FontSlot;
use nclip_ctl::Control as _;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

/// 동기화 테스트 결과 슬롯 — (주소, 핀 앞 8자) 또는 실패 사유.
type SyncTestSlot = std::sync::Arc<std::sync::Mutex<Option<Result<(String, String), String>>>>;

pub(crate) struct App {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    scale: f32,
    /// 값 + 파일 — ★ 즉시 적용이 **디스크까지** 간다([`crate::conf`]).
    pub(crate) conf: Settings,
    widget: SettingsWidget,
    /// ★ 동기화 연결 테스트 결과(09-03) — 스레드가 채우고 tick이 소비한다.
    sync_test: Option<SyncTestSlot>,
    /// ★ Test 성공 → 러너 재기동 요청(09-03) — 셸이 소비한다(`take_sync_respawn`).
    sync_respawn: bool,
    /// ★ 릴레이 None 자동 재기동 예약(09-05 사용자 — VM 실기: 핸들·암호를 **뒤늦게** 채우면 Test·재시작 전엔
    ///   러너가 안 섰다). 자유 문자열 행은 글자마다 오므로 마지막 입력 뒤 잠잠해지면 한 번만 건다(디바운스).
    sync_respawn_at: Option<Instant>,
    /// ★ 기기 이름 재소개 예약(09-05) — 연속 입력은 마지막만(설정 저장 quiet 1s와 같은 박자).
    name_announce_at: Option<Instant>,
    /// ★ 기록 모두 삭제 무장(09-04 사용자 — 2단계 확인): 첫 클릭 시각 · 2초 지나면 풀린다.
    clear_arm: Option<Instant>,
    /// ★ 둘째 클릭 확정 → 셸이 소비해 실제로 지운다(`take_clear_history`).
    clear_request: bool,
    /// 마지막으로 노트에 반영한 러너 상태(변화 때만 갱신 — 자동 Test 표시).
    sync_shown: Option<crate::sync_cmd::SyncStatus>,
    /// 마지막으로 그린 기기 목록 텍스트(변화 때만 set_value).
    devices_text: String,
    mods: ModifiersState,
    started: Instant,
    /// 마지막으로 준 크기 — 바뀔 때만 `set_bounds`를 부른다.
    laid_out: (i32, i32),
    /// 마지막 커서 위치(winit은 클릭 이벤트에 좌표를 싣지 않는다).
    cursor: (i32, i32),
    /// ★ 지금 좌우 리사이즈 커서를 보이고 있는가 — 바뀔 때만 OS에 전달한다.
    col_resize: bool,
    /// ★ 언어 등 UI 전역이 바뀌었다 — 셸이 가져다 트레이·창 전부 갱신(1회성).
    ui_refresh: bool,
    /// ★ 상주 모드(트레이 셸 안 — T-12e2) — 닫기가 `ui.close_to_tray`를 따른다.
    ///   단독 `settings` 명령에서는 거짓 — 닫기 = 종료(설정과 무관).
    resident: bool,
    /// ★ 배치 앵커(09-04 사용자) — 열 때 셸이 준 메인창 기하(x, y, w, h). 없으면 주 모니터.
    anchor: Option<(i32, i32, u32, u32)>,
    /// ★ 메인창이 최상위면 설정 창도 최상위(가려지지 않게).
    on_top: bool,
    /// ★ 단축키 캡처 오버레이(09-04 사용자 — 설정 창 안 모달): 대상 키 · 지금까지 누른 조합 · 버튼 셋.
    capture: Option<HotkeyCapture>,
}

/// 캡처 오버레이 상태.
struct HotkeyCapture {
    key: &'static str,
    combo: Option<nclip_core::hotkey::Hotkey>,
    /// 수정 키 없는 조합을 눌렀다 — 안내 문구.
    need_mod: bool,
    remove: nclip_ctl::controls::Button,
    ok: nclip_ctl::controls::Button,
    cancel: nclip_ctl::controls::Button,
}

impl App {
    pub(crate) fn new(font: Font, conf: Settings, resident: bool) -> Self {
        // 연결 상태 노트는 `poll_sync_status`가 첫 틱에 채운다(09-03 — 자동 Test 표시).
        let widget = SettingsWidget::new(&conf.state);
        let theme = crate::conf::current_theme(conf.state.get("ui.theme"));
        Self {
            window: None,
            ctx: None,
            surface: None,
            font,
            theme,
            scale: 1.0,
            conf,
            widget,
            sync_test: None,
            sync_respawn: false,
            sync_respawn_at: None,
            name_announce_at: None,
            clear_arm: None,
            clear_request: false,
            sync_shown: None,
            devices_text: String::new(),
            mods: ModifiersState::empty(),
            started: Instant::now(),
            laid_out: (0, 0),
            cursor: (0, 0),
            col_resize: false,
            ui_refresh: false,
            resident,
            anchor: None,
            on_top: false,
            capture: None,
        }
    }

    /// UI 전역 변경(언어 등) 1회성 수거 — 셸이 트레이/창 라벨을 새 언어로.
    pub(crate) fn take_ui_refresh(&mut self) -> bool {
        std::mem::take(&mut self.ui_refresh)
    }

    /// ★ Test 성공 뒤 러너 재기동이 필요한가 — 셸이 `spawn_if_enabled`로 잇는다(09-03).
    pub(crate) fn take_sync_respawn(&mut self) -> bool {
        std::mem::take(&mut self.sync_respawn)
    }

    /// ★ 릴레이 None 자동 재기동 무장(09-05 사용자) — 핸들·암호 행이 바뀔 때 부른다.
    ///   조건이 맞으면 `LAN_RESPAWN_DEBOUNCE` 뒤로 예약(연속 입력은 마지막만 남는다 — DR-41) ·
    ///   조건이 깨지면(둘 중 하나 비움 등) 예약 취소.
    fn arm_lan_respawn(&mut self, now: Instant) {
        let ready = lan_auto_respawn_ready(
            self.conf.state.get("sync.relay"),
            self.conf.state.get("sync.enabled"),
            self.conf.state.get("sync.handle"),
            self.conf.state.get("sync.passphrase"),
        );
        self.sync_respawn_at = ready.then(|| now + LAN_RESPAWN_DEBOUNCE);
    }

    /// 예약이 만료됐으면 셸에 재기동을 요청한다(tick 경로).
    fn fire_lan_respawn(&mut self) {
        if self.sync_respawn_at.is_some_and(|at| Instant::now() >= at) {
            self.sync_respawn_at = None;
            self.sync_respawn = true;
            self.sync_shown = None; // 러너가 서면 LanOnly 노트로 갱신
        }
        // ★ 이름 재소개(09-05) — 디바운스 만료에 살아 있는 세션 전부(릴레이·LAN · 승인 무관)로.
        if self.name_announce_at.is_some_and(|at| Instant::now() >= at) {
            self.name_announce_at = None;
            let n = crate::sync_cmd::announce_name();
            println!("동기화: 기기 이름 재소개 → 세션 {n}개 (끊긴 기기는 다음 접속의 첫 인사로)");
        }
    }

    /// ★ 기록 모두 삭제 확정(09-04) — 셸이 가져가 고정 제외 전부 지운다(1회성).
    pub(crate) fn take_clear_history(&mut self) -> bool {
        std::mem::take(&mut self.clear_request)
    }

    /// ★ 삭제 무장 만료(2초) — 노트를 지운다. 폴마다 값싸게.
    fn expire_clear_arm(&mut self) {
        if self
            .clear_arm
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(2))
        {
            self.clear_arm = None;
            let mut inv = Invalidations::default();
            self.widget.set_row_note("hist.clear", "", &mut inv);
            self.widget.set_action_tone(
                "hist.clear",
                nclip_ctl::controls::ButtonTone::Default,
                &mut inv,
            );
            self.redraw();
        }
    }

    /// ★ 즉시 연결 해제(09-03 사용자) — Disconnect 버튼·연결 정보 변경·Enable 끔 공용.
    ///   Connected 메시지 자리(sync.test)에 Disconnected를 표시하고 시드도 지운다.
    fn sync_drop_now(&mut self) {
        if !crate::sync_cmd::is_connected() && !crate::sync_cmd::has_lan_peers() {
            return;
        }
        crate::sync_cmd::request_disconnect();
        crate::sync_cmd::clear_last_ok();
        let lang = nclip_core::current_lang();
        let mut inv = Invalidations::default();
        self.widget.set_row_note_toned(
            "sync.test",
            nclip_core::tr(lang, nclip_core::Msg::StSyncDisconnected),
            nclip_ui::NoteTone::Info,
            &mut inv,
        );
        self.redraw();
    }

    /// 창이 없으면 만들고, 있으면 앞으로 가져온다(트레이 "열기"의 재진입 경로).
    /// ★ 캡처 오버레이 열기(09-04).
    fn begin_capture(&mut self, key: &'static str) {
        let lang = nclip_core::current_lang();
        use nclip_ctl::controls::{Button, ButtonTone};
        let mk = |m: nclip_core::Msg, tone: ButtonTone| {
            let mut b = Button::new(nclip_core::tr(lang, m)).with_tone(tone);
            b.set_scale(self.scale);
            b
        };
        self.capture = Some(HotkeyCapture {
            key,
            combo: None,
            need_mod: false,
            remove: mk(nclip_core::Msg::HotkeyRemove, ButtonTone::Danger),
            ok: mk(nclip_core::Msg::HotkeyOk, ButtonTone::Safe),
            cancel: mk(nclip_core::Msg::HotkeyCancel, ButtonTone::Default),
        });
        self.redraw();
    }

    /// 오버레이 패널·버튼 자리(창 크기 기준 · 매 페인트 계산 — 값싸다).
    fn capture_layout_of(&self, w: i32, h: i32) -> (Rect, Rect, Rect, Rect) {
        capture_layout(self.scale, w, h)
    }
}

/// 오버레이 패널·버튼 자리 — (패널, 제거, 확인, 취소).
fn capture_layout(scale: f32, w: i32, h: i32) -> (Rect, Rect, Rect, Rect) {
    {
        let px = |v: f32| (v * scale).round() as i32;
        let (pw, ph) = (px(380.0).min(w - px(20.0)), px(190.0));
        let panel = Rect::new((w - pw) / 2, (h - ph) / 2, pw, ph);
        let (bw, bh, gap, pad) = (px(96.0), px(30.0), px(8.0), px(14.0));
        let by = panel.y + ph - pad - bh;
        let remove = Rect::new(panel.x + pad, by, px(120.0), bh);
        let cancel = Rect::new(panel.x + pw - pad - bw, by, bw, bh);
        let ok = Rect::new(cancel.x - gap - bw, by, bw, bh);
        (panel, remove, ok, cancel)
    }
}

impl App {
    /// 캡처 확정·제거·취소 — 값 반영은 설정 + 위젯 라벨.
    fn end_capture(&mut self, apply: Option<String>) {
        let Some(c) = self.capture.take() else {
            return;
        };
        if let Some(v) = apply {
            let now = Instant::now();
            self.conf.set(c.key, v.clone(), now);
            let mut inv = Invalidations::default();
            self.widget.set_value(c.key, &v, &mut inv);
            println!(
                "단축키 변경: {} = {}",
                c.key,
                if v.is_empty() { "없음" } else { v.as_str() }
            );
        }
        self.redraw();
    }

    /// 캡처 중 키 입력 — 수정 키 단독은 무시 · Esc 취소 · Enter 확정 · 그 외 = 조합 갱신.
    fn capture_key(&mut self, el: &ActiveEventLoop, event: &winit::event::KeyEvent) {
        let _ = el;
        if event.state != ElementState::Pressed {
            return;
        }
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        let meta = self.mods.super_key();
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => {
                self.end_capture(None);
                return;
            }
            Key::Named(NamedKey::Enter) if !ctrl && !shift && !alt && !meta => {
                let v = self
                    .capture
                    .as_ref()
                    .and_then(|c| c.combo.map(|h| h.canonical()));
                if v.is_some() {
                    self.end_capture(v);
                }
                return;
            }
            _ => {}
        }
        let Some(tok) = keycode_token(&event.physical_key) else {
            return; // 수정 키 단독·미지원 키
        };
        let Some(c) = self.capture.as_mut() else {
            return;
        };
        match nclip_core::hotkey::Hotkey::from_parts(ctrl, shift, alt, meta, tok) {
            Some(h) if h.is_global_safe() => {
                c.combo = Some(h);
                c.need_mod = false;
            }
            Some(_) => {
                c.combo = None;
                c.need_mod = true;
            }
            None => {}
        }
        self.redraw();
    }

    /// 캡처 중 마우스 — 버튼 셋만 받는다.
    fn capture_mouse(&mut self, ev: InputEvent) {
        let Some(win) = &self.window else {
            return;
        };
        let size = win.inner_size();
        let (_, r_remove, r_ok, r_cancel) =
            self.capture_layout_of(size.width as i32, size.height as i32);
        let Some(c) = self.capture.as_mut() else {
            return;
        };
        let mut inv = Invalidations::default();
        c.remove.set_bounds(r_remove, &mut inv);
        c.ok.set_bounds(r_ok, &mut inv);
        c.cancel.set_bounds(r_cancel, &mut inv);
        c.remove.on_event(&ev, &mut inv);
        c.ok.on_event(&ev, &mut inv);
        c.cancel.on_event(&ev, &mut inv);
        let (rm, ok, cancel) = (
            c.remove.take_clicked(),
            c.ok.take_clicked(),
            c.cancel.take_clicked(),
        );
        let combo = c.combo.map(|h| h.canonical());
        if rm {
            self.end_capture(Some(String::new()));
        } else if ok {
            if combo.is_some() {
                self.end_capture(combo);
            }
        } else if cancel {
            self.end_capture(None);
        } else {
            self.redraw();
        }
    }
}

/// 오버레이 그리기(위젯 위 · 맨 끝) — 자유 함수(페인트 중 surface 대여와 겹치지 않게 필드만 받는다).
fn paint_capture(
    c: &mut HotkeyCapture,
    dc: &mut RasterCtx<'_, '_, '_>,
    w: i32,
    h: i32,
    th: &Theme,
    scale: f32,
) {
    {
        let (panel, r_remove, r_ok, r_cancel) = capture_layout(scale, w, h);
        let th = *th;
        let lang = nclip_core::current_lang();
        let px = |v: f32| (v * scale).round() as i32;
        dc.fill_rect_alpha(
            Rect::new(0, 0, w, h),
            nclip_ctl::theme::Color::from_rgb(0, 0, 0),
            0.35,
        );
        dc.fill_round_rect(panel, px(8.0), th.window_bg);
        dc.stroke_round_rect(panel, px(8.0), th.border, 1.0);
        let full = Rect::new(0, 0, w, h);
        dc.select_font(FontSlot::Base, true);
        dc.text(
            panel.x + px(14.0),
            panel.y + px(12.0),
            full,
            nclip_core::tr(lang, nclip_core::Msg::HotkeyTitle),
            th.text,
        );
        dc.select_font(FontSlot::Status, false);
        let prompt = if c.need_mod {
            nclip_core::tr(lang, nclip_core::Msg::HotkeyNeedMod)
        } else {
            nclip_core::tr(lang, nclip_core::Msg::HotkeyPrompt)
        };
        dc.text(
            panel.x + px(14.0),
            panel.y + px(40.0),
            full,
            prompt,
            if c.need_mod { th.warn } else { th.text_dim },
        );
        // 조합 표시 상자 — 검색창 높이 · accent 글자.
        let boxr = Rect::new(
            panel.x + px(14.0),
            panel.y + px(66.0),
            panel.w - px(28.0),
            px(34.0),
        );
        dc.fill_round_rect(boxr, px(5.0), th.field_bg);
        dc.stroke_round_rect(boxr, px(5.0), th.accent, 1.0);
        let shown = c
            .combo
            .map(|hk| hk.display(cfg!(target_os = "macos")))
            .unwrap_or_default();
        dc.select_font_sized(FontSlot::Base, true, 2.0 * scale);
        dc.text(boxr.x + px(10.0), boxr.y + px(6.0), full, &shown, th.accent);
        dc.select_font(FontSlot::Base, false);
        let mut inv = Invalidations::default();
        c.remove.set_bounds(r_remove, &mut inv);
        c.ok.set_bounds(r_ok, &mut inv);
        c.cancel.set_bounds(r_cancel, &mut inv);
        c.remove.paint(dc, &th);
        c.ok.paint(dc, &th);
        c.cancel.paint(dc, &th);
    }
}

impl App {
    /// ★ 열기 직전 셸이 준다(09-04) — 메인창 기하 + 메인창 최상위 여부.
    pub(crate) fn set_anchor(&mut self, main_geom: Option<(i32, i32, u32, u32)>, on_top: bool) {
        self.anchor = main_geom;
        self.set_level(on_top);
    }

    /// ★ 창 레벨(09-04) — 메인창 최상위 토글을 따라간다.
    pub(crate) fn set_level(&mut self, on_top: bool) {
        self.on_top = on_top;
        if let Some(w) = &self.window {
            w.set_window_level(if on_top {
                winit::window::WindowLevel::AlwaysOnTop
            } else {
                winit::window::WindowLevel::Normal
            });
        }
    }

    /// ★ 첫 위치(09-04 사용자): 메인창이 있는 모니터에서 — 같은 모니터에 저장된 마지막 위치가 있으면 그것,
    ///   아니면(첫 실행 · 메인창이 모니터를 옮김) 메인창 오른쪽 → 안 들어가면 왼쪽 → 그래도 안 되면 메인창 위에 살짝 겹쳐.
    ///   모니터 안으로 죈다. 반환 = (물리 좌표, 물리 크기, 모니터 키).
    fn initial_placement(&self, el: &ActiveEventLoop) -> Option<Placement> {
        let (ax, ay, aw, ah) = self.anchor.unwrap_or((0, 0, 0, 0));
        #[allow(clippy::cast_possible_wrap)]
        let (acx, acy) = (ax + aw as i32 / 2, ay + ah as i32 / 2);
        let contains = |m: &winit::monitor::MonitorHandle| {
            let p = m.position();
            let sz = m.size();
            acx >= p.x
                && acy >= p.y
                && acx < p.x + i32::try_from(sz.width).unwrap_or(i32::MAX)
                && acy < p.y + i32::try_from(sz.height).unwrap_or(i32::MAX)
        };
        let mon = if self.anchor.is_some() {
            el.available_monitors().find(contains)
        } else {
            None
        }
        .or_else(|| el.primary_monitor())?;
        let key = monitor_key(&mon);
        let (mp, ms) = (mon.position(), mon.size());
        #[allow(clippy::cast_possible_wrap)]
        let (mx, my, mw, mh) = (mp.x, mp.y, ms.width as i32, ms.height as i32);
        let scale = mon.scale_factor();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (sw, sh) = {
            let gw = self.conf.state.get("ui.set_w").parse::<u32>().unwrap_or(0);
            let gh = self.conf.state.get("ui.set_h").parse::<u32>().unwrap_or(0);
            if gw >= 320 && gh >= 240 {
                (gw, gh)
            } else {
                (
                    (760.0 * scale).round() as u32,
                    (560.0 * scale).round() as u32,
                )
            }
        };
        #[allow(clippy::cast_possible_wrap)]
        let (swi, shi) = (sw as i32, sh as i32);
        let clamp = |x: i32, y: i32| -> (i32, i32) {
            (
                x.clamp(mx, (mx + mw - swi).max(mx)),
                y.clamp(my, (my + mh - shi).max(my)),
            )
        };
        // 같은 모니터에 저장된 마지막 위치 → 복원. 모니터가 다르면(메인창 이동) 버린다 = 초기화.
        let saved_mon = self.conf.state.get("ui.set_mon").to_string();
        let sx = self.conf.state.get("ui.set_x").parse::<i32>().ok();
        let sy = self.conf.state.get("ui.set_y").parse::<i32>().ok();
        if saved_mon == key {
            if let (Some(x), Some(y)) = (sx, sy) {
                return Some((clamp(x, y), (sw, sh), key));
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        let gap = (12.0 * scale).round() as i32;
        let (x, y) = if self.anchor.is_some() {
            #[allow(clippy::cast_possible_wrap)]
            let right = ax + aw as i32 + gap;
            if right + swi <= mx + mw {
                (right, ay)
            } else if ax - gap - swi >= mx {
                (ax - gap - swi, ay)
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let off = (40.0 * scale).round() as i32;
                (ax + off, ay + off)
            }
        } else {
            (mx + (mw - swi) / 2, my + (mh - shi) / 2)
        };
        Some((clamp(x, y), (sw, sh), key))
    }

    /// ★ 위치·크기·모니터 저장(09-04) — 닫을 때·종료 때.
    fn save_geom(&mut self) {
        let Some(w) = &self.window else {
            return;
        };
        let Ok(pos) = w.outer_position() else {
            return;
        };
        let size = w.inner_size();
        let key = w
            .current_monitor()
            .map(|m| monitor_key(&m))
            .unwrap_or_default();
        let now = Instant::now();
        self.conf.set("ui.set_x", pos.x.to_string(), now);
        self.conf.set("ui.set_y", pos.y.to_string(), now);
        self.conf.set("ui.set_w", size.width.to_string(), now);
        self.conf.set("ui.set_h", size.height.to_string(), now);
        self.conf.set("ui.set_mon", key, now);
    }

    pub(crate) fn ensure_window(&mut self, el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.set_visible(true);
            // ★ Wayland는 `set_visible`/`focus_window`가 no-op(xdg-shell — 클라이언트는 창을
            //   되살릴 수 없다). 셸이 트레이 클릭 때 준 활성화 토큰이 있으면 **진짜 포커스**,
            //   없으면 주의 요청(Dock 강조 → 사용자가 한 번 클릭). beep 08-29 실기 그대로.
            #[cfg(target_os = "linux")]
            {
                w.set_minimized(false);
                // ★ Wayland = 셸 토큰으로 활성화 · X11 = 페이저 소스 올리기(09-05). 둘 다 못 하면
                //   그때만 주의 요청(= "준비됨" 알림) — X11에서 이 알림이 뜨던 것을 없앤다.
                let activated = nclip_plat::tray::take_activation_token()
                    .is_some_and(|tok| wayland_activate(w, &tok))
                    || linux_raise_x11(w);
                if !activated {
                    w.request_user_attention(Some(winit::window::UserAttentionType::Critical));
                }
            }
            w.focus_window();
            bring_to_front(w);
            return;
        }
        // ★ 창 아이콘(08-31 사용자 실기 "작업표시줄에 일반 창 아이콘") — 트레이와 같은 그림.
        let attrs = win_name(crate::icon::with_icon(
            Window::default_attributes()
                // Linux(GNOME)는 서버 장식이 없어 winit(sctk-adwaita)이 제목을 자체 폰트로
                // 그리는데 한글 글리프가 없다(08-30 사용자 실기 "타이틀바 글씨 깨짐") → ASCII.
                .with_title(if cfg!(target_os = "linux") {
                    "Nexa Clip - Settings"
                } else {
                    "Nexa Clip — 설정 (검색 · 사이드바 경계 드래그 · Esc 종료)"
                })
                .with_inner_size(winit::dpi::LogicalSize::new(760.0, 560.0)),
        ));
        // ★ 배치(09-04) — 메인창 모니터 · 메인창 옆 · 같은 모니터의 마지막 위치.
        let placement = self.initial_placement(el);
        let attrs = match &placement {
            Some(((x, y), (w, h), _)) => attrs
                .with_position(winit::dpi::PhysicalPosition::new(*x, *y))
                .with_inner_size(winit::dpi::PhysicalSize::new(*w, *h)),
            None => attrs,
        };
        // 새 창도 토큰이 있으면 그것으로 활성화한다(트레이 → 첫 열기).
        #[cfg(target_os = "linux")]
        let attrs = {
            use winit::platform::startup_notify::WindowAttributesExtStartupNotify as _;
            match nclip_plat::tray::take_activation_token() {
                Some(tok) => {
                    attrs.with_activation_token(winit::window::ActivationToken::from_raw(tok))
                }
                None => attrs,
            }
        };
        let Ok(win) = el.create_window(attrs) else {
            eprintln!("창 생성 실패");
            if !self.resident {
                el.exit();
            }
            return;
        };
        let win = Rc::new(win);
        // ★ 메인창이 최상위면 설정 창도 최상위(09-04 사용자 — 메인창 뒤로 숨지 않게).
        win.set_window_level(if self.on_top {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        });
        if let Some((_, _, key)) = &placement {
            let now = Instant::now();
            self.conf.set("ui.set_mon", key.clone(), now);
        }
        bring_to_front(&win);
        self.scale = win.scale_factor() as f32;
        let mut inv = Invalidations::default();
        self.widget.set_scale(self.scale, &mut inv);
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
        self.laid_out = (0, 0);
        self.window = Some(win);
    }

    /// 닫기 요청 — ★ 상주 모드에서 `ui.close_to_tray`가 켜져 있으면 **종료 대신 숨는다**
    /// (08-28 사용자 요청 · beep과 동일 · 기본 꺼짐 = X는 종료).
    fn request_close(&mut self, el: &ActiveEventLoop) {
        if self.resident && self.conf.state.get("ui.close_to_tray") == "on" {
            self.hide_window();
        } else {
            self.save_geom();
            el.exit();
        }
    }

    /// 창을 걷는다(상주는 유지) — ★ 미저장 변경은 여기서 강제 수거한다.
    fn hide_window(&mut self) {
        self.save_geom();
        self.surface = None;
        self.ctx = None;
        self.window = None;
        self.laid_out = (0, 0);
        self.col_resize = false;
        if self.conf.flush() {
            println!("설정 저장: {}", self.conf.path().display());
        }
        println!("창을 트레이로 숨겼습니다 — 트레이 좌클릭·열기로 다시 엽니다.");
    }

    /// OS 주 수식키 — mac은 ⌘, 나머지는 Ctrl([docs/14 §6] 관례).
    fn primary(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.mods.super_key()
        } else {
            self.mods.control_key()
        }
    }

    /// ★ 동기화 연결 테스트(09-03) — 지금 설정값으로 릴레이에 실제 접속(스레드).
    fn start_sync_test(&mut self) {
        let lang = nclip_core::current_lang();
        let mut inv = Invalidations::default();
        self.widget.set_row_note_toned(
            "sync.test",
            nclip_core::tr(lang, nclip_core::Msg::StSyncTesting),
            nclip_ui::NoteTone::Info,
            &mut inv,
        );
        self.redraw();
        let addr = {
            let a = self.conf.state.get("sync.relay").trim().to_string();
            if a.is_empty() {
                "beepd.sosomlab.com".to_string()
            } else {
                a
            }
        };
        // ★ 핸들·암호가 비면 시험도 접속도 없다(09-04 사용자) — 노트로 이유만.
        let identity_ok = !self.conf.state.get("sync.handle").trim().is_empty()
            && !self.conf.state.get("sync.passphrase").trim().is_empty();
        if !identity_ok {
            self.widget.set_row_note_toned(
                "sync.test",
                nclip_core::tr(lang, nclip_core::Msg::StSyncNeedIdentity),
                nclip_ui::NoteTone::Warn,
                &mut inv,
            );
            self.redraw();
            return;
        }
        // ★ 릴레이 None(09-04) — 서버 시험 없이 LAN 전용으로 켜고 러너(LAN)를 띄운다.
        if addr == "none" {
            if self.conf.state.get("sync.enabled") != "on" {
                self.conf
                    .set("sync.enabled", "on".to_string(), Instant::now());
                self.widget.set_value("sync.enabled", "on", &mut inv);
            }
            self.sync_respawn = true;
            self.sync_shown = None; // LanOnly 상태를 노트로
            self.redraw();
            return;
        }
        let port = {
            let p = self.conf.state.get("sync.port").trim().to_string();
            if p.is_empty() {
                "47300".to_string()
            } else {
                p
            }
        };
        let raw = format!("{addr}:{port}");
        let dir = crate::conf::data_dir();
        // ★ 연결 정보가 바뀌었을 수 있다(09-03 사용자) — 연결 중이면 끊고 새로 시도.
        if crate::sync_cmd::is_connected() {
            crate::sync_cmd::request_disconnect();
            crate::sync_cmd::clear_last_ok();
        }
        let slot: SyncTestSlot = std::sync::Arc::new(std::sync::Mutex::new(None));
        self.sync_test = Some(slot.clone());
        std::thread::Builder::new()
            .name("nclip-sync-test".into())
            .spawn(move || {
                // 러너가 물러날 때까지 잠깐 대기 — 같은 신원 RID 이중 접속을 피한다.
                for _ in 0..50 {
                    if !crate::sync_cmd::is_running() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                let out = sync_test_once(&raw, &dir);
                if let Ok(mut g) = slot.lock() {
                    *g = Some(out);
                }
            })
            .ok();
    }

    /// 테스트 결과 소비 — tick 경로에서 부른다(끝났으면 노트 갱신).
    /// ★ 러너 상태 → Test 행 노트 자동 반영(09-03 사용자 — "실행 시 자동 Test").
    ///   수동 Test가 진행 중이면 그 결과가 우선(끝나면 러너 상태가 이어받는다).
    fn poll_sync_status(&mut self) {
        self.refresh_devices();
        self.expire_clear_arm();
        // ★ 검색 방식은 창 밖(드롭다운)에서도 바뀐다(09-04) — 설정값을 라디오에 되비춘다(같으면 무효화 없음).
        {
            let mode = self.conf.state.get("find.mode").to_string();
            let mut inv = Invalidations::default();
            self.widget.set_value("find.mode", &mode, &mut inv);
        }
        use crate::sync_cmd::SyncStatus as S;
        let st = crate::sync_cmd::status();
        // ★ 활성 규칙(09-03·09-04 사용자): 동기화가 꺼져 있으면 **아래 설정 전부 잠금**(의미가 없다) ·
        //   켜져 있으면 연결 중엔 Test 잠금(정보 변경 = 해제 → 다시 열림) · Disconnect는 연결 중에만.
        //   매 폴마다 계산(값싸다 · set_disabled는 바뀔 때만 무효화).
        let enabled = self.conf.state.get("sync.enabled") == "on";
        let relay_none = self.conf.state.get("sync.relay").trim() == "none";
        // ★ 핸들·암호 둘 다 있어야 접속이 가능하다(09-04 사용자) — 비면 Test·Disconnect 잠금.
        let identity_ok = !self.conf.state.get("sync.handle").trim().is_empty()
            && !self.conf.state.get("sync.passphrase").trim().is_empty();
        let locked: &[&'static str] = if !enabled {
            &[
                "sync.device_name",
                "sync.handle",
                "sync.passphrase",
                "sync.relay",
                "sync.port",
                "sync.retry",
                "sync.test",
                "sync.disconnect",
                "sync.devices",
            ]
        } else if !identity_ok {
            &["sync.test", "sync.disconnect"]
        } else if relay_none {
            // ★ 릴레이 None(09-04 사용자) — 서버가 없으니 포트·Test·Disconnect는 의미가 없다.
            &["sync.port", "sync.test", "sync.disconnect"]
        } else if st == S::Connected {
            &["sync.test"]
        } else {
            &["sync.disconnect"]
        };
        {
            let mut inv0 = Invalidations::default();
            self.widget.set_disabled(locked, &mut inv0);
            if !inv0.is_empty() {
                self.redraw();
            }
        }
        if self.sync_test.is_some() {
            return;
        }
        if self.sync_shown.as_ref() == Some(&st) {
            return;
        }
        let lang = nclip_core::current_lang();
        let mut inv = Invalidations::default();
        let (msg, tone) = match &st {
            S::Off => (String::new(), nclip_ui::NoteTone::Plain),
            S::Connecting => (
                nclip_core::tr(lang, nclip_core::Msg::StSyncTesting).to_string(),
                nclip_ui::NoteTone::Info,
            ),
            S::LanOnly => (
                nclip_core::tr(lang, nclip_core::Msg::StSyncLanOnly).to_string(),
                nclip_ui::NoteTone::Ok,
            ),
            S::Connected => {
                let (raw, pin8) = crate::sync_cmd::last_ok().unwrap_or_default();
                (
                    nclip_core::tr(lang, nclip_core::Msg::StSyncTestOk)
                        .replacen("{}", &raw, 1)
                        .replacen("{}", &pin8, 1),
                    nclip_ui::NoteTone::Ok,
                )
            }
            S::Failed(e) => (
                nclip_core::tr(lang, nclip_core::Msg::StSyncTestFail).replacen("{}", e, 1),
                nclip_ui::NoteTone::Warn,
            ),
            S::Stopped => (
                nclip_core::tr(lang, nclip_core::Msg::StSyncDisconnected).to_string(),
                nclip_ui::NoteTone::Info,
            ),
            S::Unconfigured => (
                nclip_core::tr(lang, nclip_core::Msg::StSyncNeedIdentity).to_string(),
                nclip_ui::NoteTone::Warn,
            ),
        };
        self.widget
            .set_row_note_toned("sync.test", &msg, tone, &mut inv);
        self.sync_shown = Some(st);
        self.redraw();
    }

    /// ★ 기기 목록 행(09-03) — 이 기기 + 만난 기기(이름 · 지문 8자 · OS · 연결/마지막 접속).
    fn refresh_devices(&mut self) {
        let lang = nclip_core::current_lang();
        let mut lines = Vec::new();
        let me = crate::sync_cmd::my_hex();
        if me.len() >= 8 {
            lines.push(format!(
                "{me}\tme\t{}: {} · {}",
                nclip_core::tr(lang, nclip_core::Msg::StSyncDevMe),
                crate::sync_cmd::my_display_name(),
                &me[..8]
            ));
        }
        let me_peer = nclip_sync::relay::parse_peer_hex(&me);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for d in crate::devices::list() {
            let short = if d.hex.len() >= 8 {
                &d.hex[..8]
            } else {
                d.hex.as_str()
            };
            let appr = nclip_core::tr(
                lang,
                if d.approved {
                    nclip_core::Msg::StSyncDevApproved
                } else {
                    nclip_core::Msg::StSyncDevNeedsApproval
                },
            );
            let state = if d.approved { "approved" } else { "pending" };
            // ★ 6자리 대조 코드(docs/09 §6-3 "입력이 아니라 보기") — 두 키의 안전번호 앞 6자리(양쪽 동일).
            let sas = me_peer
                .zip(nclip_sync::relay::parse_peer_hex(&d.hex))
                .map(|(mp, p)| {
                    let n: String = nclip_sync::sas::safety_number(mp, p)
                        .chars()
                        .filter(char::is_ascii_digit)
                        .take(6)
                        .collect();
                    format!(
                        " · {}",
                        nclip_core::tr(lang, nclip_core::Msg::StSyncSas).replacen("{}", &n, 1)
                    )
                })
                .unwrap_or_default();
            if d.online {
                lines.push(format!(
                    "{}\t{state}\t{} · {} · {} · {} ({}) · {}{sas}",
                    d.hex,
                    d.name,
                    short,
                    d.os,
                    nclip_core::tr(lang, nclip_core::Msg::StSyncDevOnline),
                    d.via,
                    appr
                ));
            } else {
                let ago = now.saturating_sub(d.last_seen);
                let ago = if ago < 3600 {
                    format!("{}m", ago / 60)
                } else if ago < 86_400 {
                    format!("{}h", ago / 3600)
                } else {
                    format!("{}d", ago / 86_400)
                };
                lines.push(format!(
                    "{}\t{state}\t{} · {} · {} · {} · {}",
                    d.hex,
                    d.name,
                    short,
                    d.os,
                    nclip_core::tr(lang, nclip_core::Msg::StSyncDevAgo).replacen("{}", &ago, 1),
                    appr
                ));
            }
        }
        let text = lines.join("\n");
        if text != self.devices_text {
            self.devices_text = text.clone();
            let mut inv = Invalidations::default();
            self.widget.set_value("sync.devices", &text, &mut inv);
            self.laid_out = (0, 0); // 줄 수가 바뀌면 행 높이도 바뀐다.
            self.redraw();
        }
    }

    fn poll_sync_test(&mut self) {
        let Some(slot) = &self.sync_test else { return };
        let done = slot.lock().ok().and_then(|mut g| g.take());
        let Some(res) = done else { return };
        self.sync_test = None;
        self.sync_shown = None;
        let lang = nclip_core::current_lang();
        let mut inv = Invalidations::default();
        match res {
            Ok((raw, pin8)) => {
                let msg = nclip_core::tr(lang, nclip_core::Msg::StSyncTestOk)
                    .replacen("{}", &raw, 1)
                    .replacen("{}", &pin8, 1);
                self.widget
                    .set_row_note_toned("sync.test", &msg, nclip_ui::NoteTone::Ok, &mut inv);
                println!("동기화 테스트: 성공 — {raw} · 핀 {pin8}");
                // ★ 성공 = 자동 접속 계약(09-03 사용자 — "실행 시 자동 접속으로 동일 상태"):
                //   sync.enabled를 켜서 다음 시작부터 러너가 같은 상태를 만든다.
                crate::sync_cmd::note_last_ok(&raw, &pin8);
                // ★ 성공 = 접속 유지 계약(09-03) — 셸이 러너를 (재)기동한다.
                self.sync_respawn = true;
                if self.conf.state.get("sync.enabled") != "on" {
                    self.conf
                        .set("sync.enabled", "on".to_string(), Instant::now());
                    // 토글 역반영(재구성하면 첫 카테고리로 튄다 — 09-03 실기).
                    self.widget.set_value("sync.enabled", "on", &mut inv);
                    println!("동기화: 테스트 성공 → 자동 접속 켜짐(sync.enabled = on)");
                }
            }
            Err(e) => {
                let msg =
                    nclip_core::tr(lang, nclip_core::Msg::StSyncTestFail).replacen("{}", &e, 1);
                self.widget.set_row_note_toned(
                    "sync.test",
                    &msg,
                    nclip_ui::NoteTone::Warn,
                    &mut inv,
                );
                eprintln!("동기화 테스트: 실패 — {e}");
            }
        }
        self.redraw();
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// 위젯에 이벤트를 넘기고, **바뀐 값이 있으면 즉시 반영**한다(설정에 저장 버튼이 없는 이유).
    fn feed(&mut self, ev: InputEvent) {
        let mut inv = Invalidations::default();
        self.widget.on_event(&ev, &mut inv);
        self.drain_edit_ctx(&mut inv);
        self.drain_changes();
        if !inv.is_empty() {
            self.redraw();
        }
    }

    /// ★ 텍스트 입력 우클릭 메뉴의 선택을 실행한다(09-03 사용자 — 암호 입력란 편집 기능):
    ///   클립보드 접근은 호스트 몫(main_win `drain_search_edit_ctx`와 같은 경로).
    fn drain_edit_ctx(&mut self, inv: &mut Invalidations) {
        if let Some(act) = self.widget.take_edit_ctx() {
            use nclip_ctl::controls::EditCtxAction as A;
            match act {
                A::Copy => {
                    if let Some(t) = self.widget.clipboard_copy() {
                        crate::cliptext::set_text(&t);
                    }
                }
                A::Cut => {
                    if let Some(t) = self.widget.clipboard_cut(inv) {
                        crate::cliptext::set_text(&t);
                    }
                }
                A::Paste => {
                    if let Some(t) = crate::cliptext::get_text() {
                        self.widget.clipboard_paste(t.trim_end_matches('\n'), inv);
                    }
                }
            }
            inv.push(nclip_ctl::geom::Rect::new(0, 0, 1, 1)); // 다시 그리기 보장
        }
    }

    /// 현재 테마(팝업이 같은 것을 쓴다).
    pub(crate) fn theme(&self) -> Theme {
        self.theme
    }

    /// ★ `ui.theme`(system/dark/light)을 OS 선호와 함께 다시 푼다 — 설정 변경·OS 테마 변경 때.
    pub(crate) fn apply_theme(&mut self) {
        let t = crate::conf::current_theme(self.conf.state.get("ui.theme"));
        if t.is_dark != self.theme.is_dark {
            self.theme = t;
            if let Some(w) = &self.window {
                // 창 장식(CSD)도 같이 — Linux sctk 장식이 실효값을 따른다.
                w.set_theme(Some(if t.is_dark {
                    winit::window::Theme::Dark
                } else {
                    winit::window::Theme::Light
                }));
            }
            self.redraw();
        }
    }

    fn drain_changes(&mut self) {
        let now = Instant::now();
        for (key, val) in self.widget.take_changes() {
            // ★ 단축키 행 클릭(09-04) — 값이 아니라 캡처 오버레이를 연다.
            if key.starts_with("key.") && val == "run" {
                self.begin_capture(key);
                continue;
            }
            // ★ 즉시 적용 계약 — 값은 바로 반영하고, **파일 쓰기는 미룬다**
            //   (조용해진 뒤 1초 · 늦어도 10초 — [`crate::conf`]).
            self.conf.set(key, val.clone(), now);
            // 비밀값은 찍지 않는다(09-04 — 페어링 암호가 로그에 남지 않게).
            if key.contains("passphrase") {
                println!("설정 변경: {key} = ••• ({}자)", val.chars().count());
            } else {
                println!("설정 변경: {key} = {val}");
            }
            if key == "ui.theme" {
                self.apply_theme();
            }
            // ★ 언어 즉시 반영(09-02 "재시작 최소화") — 전역 언어 + 설정 위젯 재구성.
            //   셸(트레이·메인·팝업)은 take_ui_refresh로 알아채 갱신한다.
            if key == "app.lang" {
                crate::conf::apply_lang(&self.conf);
                self.widget = SettingsWidget::new(&self.conf.state);
                let mut inv2 = Invalidations::default();
                self.widget.set_scale(self.scale, &mut inv2);
                self.laid_out = (0, 0);
                self.ui_refresh = true;
                self.redraw();
            }
            // ★ 자동 시작은 값만 저장하면 아무 일도 안 일어난다 — **OS 등록까지 즉시**.
            //   실패해도 값은 유지된다(다음 부팅 동기화·재토글에서 재시도 — beep 규약).
            // ★ 핸들 입력 → 패스프레이즈 추천(09-03 사용자) — 비어 있을 때만 채운다.
            if key == "sync.handle" && !val.trim().is_empty() {
                let cur = self.conf.state.get("sync.passphrase").trim().to_string();
                if cur.is_empty() {
                    let sug = suggest_passphrase();
                    self.conf.set("sync.passphrase", sug.clone(), now);
                    // 값 역반영(마스킹 유지 · 카테고리·스크롤 유지) + 안내 노트.
                    let mut inv2 = Invalidations::default();
                    self.widget.set_value("sync.passphrase", &sug, &mut inv2);
                    let lang = nclip_core::current_lang();
                    self.widget.set_row_note_toned(
                        "sync.passphrase",
                        nclip_core::tr(lang, nclip_core::Msg::StSyncPassSuggested),
                        nclip_ui::NoteTone::Info,
                        &mut inv2,
                    );
                    self.redraw();
                }
            }
            // ★ 기기별 승인/해제/삭제(09-04 사용자 — 행별 버튼). 값 = `approve:hex|revoke:hex|delete:hex`.
            if key == "sync.devices" {
                let (verb, hex) = val.split_once(':').unwrap_or(("", ""));
                let changed = match verb {
                    "approve" => crate::devices::set_approved(hex, true),
                    "revoke" => crate::devices::set_approved(hex, false),
                    "delete" => crate::devices::remove(hex),
                    _ => false,
                };
                if changed {
                    if let Err(e) =
                        crate::devices::save(&crate::conf::data_dir().join("devices.txt"))
                    {
                        eprintln!("동기화: 기기 목록 저장 실패({e})");
                    }
                    println!("동기화: 기기 {}… {verb}", &hex[..hex.len().min(8)]);
                    self.devices_text.clear(); // 목록 재구성
                }
                continue; // 값 키가 아니다 — 아래 핸들러는 무관
            }
            // ★ 재시도 정책(09-04) — 즉시 반영(다음 실패부터 새 대기).
            if key == "sync.retry" {
                crate::sync_cmd::set_policy(&val);
            }
            // ★ 기기 이름(09-03) — 즉시 반영 + ★ 저장 박자(1s 디바운스)에 연결된 기기 전부에 재소개(09-05).
            if key == "sync.device_name" {
                crate::sync_cmd::set_device_name(&val);
                self.name_announce_at = Some(now + NAME_ANNOUNCE_DEBOUNCE);
            }
            // ★ 비밀번호 생성 버튼(09-03 사용자) — 새 패스프레이즈로 교체 + 안내.
            if key == "sync.passphrase.regen" && val == "run" {
                let sug = suggest_passphrase();
                self.conf.set("sync.passphrase", sug.clone(), now);
                // 값 역반영 — 재구성하면 첫 카테고리로 튀었다(09-03 실기 결함).
                let mut inv2 = Invalidations::default();
                self.widget.set_value("sync.passphrase", &sug, &mut inv2);
                let lang = nclip_core::current_lang();
                self.widget.set_row_note_toned(
                    "sync.passphrase",
                    nclip_core::tr(lang, nclip_core::Msg::StSyncPassSuggested),
                    nclip_ui::NoteTone::Info,
                    &mut inv2,
                );
                self.redraw();
                // 연결 정보가 바뀌었다 — 연결 중이면 즉시 해제(재접속 = Test · 릴레이 None이면 자동).
                self.sync_drop_now();
                self.arm_lan_respawn(now);
            }
            // ★ 연결 정보 변경 = 즉시 해제(09-03 사용자) — Test로 새 정보 재접속.
            if matches!(
                key,
                "sync.handle" | "sync.passphrase" | "sync.relay" | "sync.port"
            ) {
                self.sync_drop_now();
            }
            // ★ 릴레이 None(09-04 사용자) — Test 없이 바로 LAN 전용으로 적용(동기화가 켜져 있으면).
            //   None → 서버로 바꾸는 경우는 종전대로 해제 후 Test.
            let relay_none_now = self.conf.state.get("sync.relay").trim() == "none";
            if relay_none_now
                && self.conf.state.get("sync.enabled") == "on"
                && (key == "sync.relay" || key == "sync.enabled")
            {
                self.sync_respawn = true;
                self.sync_shown = None;
            }
            // ★ 릴레이 None + 핸들·암호를 **뒤늦게** 채운 경우(09-05 사용자 — VM 실기): 종전엔 Test·재시작 전까지
            //   러너가 서지 않았다(입력 순서에 따라 결과가 갈림). 둘 다 차면 잠잠해진 뒤 한 번 자동 재기동.
            if matches!(key, "sync.handle" | "sync.passphrase") {
                self.arm_lan_respawn(now);
            }
            // ★ Enable Sync 끔 = 즉시 해제(09-03 사용자).
            if key == "sync.enabled" && val != "on" {
                self.sync_drop_now(); // Connected 자리에 Disconnected 노트(연결 중이었다면)
                crate::sync_cmd::stop_all(); // ★ 릴레이·LAN·세션 전부 끔(09-04 사용자)
                self.sync_shown = None;
            }
            // ★ 연결 테스트(09-03 — beep 화법: 진행/성공/실패를 행 노트로).
            if key == "sync.test" && val == "run" {
                self.start_sync_test();
            }
            // ★ 기록 모두 삭제(09-04 사용자) — **2단계**: 첫 클릭 = 경고 노트 + 2초 무장 · 그 안의 둘째 클릭 = 확정.
            if key == "hist.clear" && val == "run" {
                let lang = nclip_core::current_lang();
                let mut inv = Invalidations::default();
                let armed = self
                    .clear_arm
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
                // ★ 버튼 자체도 무장 중엔 Danger(사용자 — 암호 재생성과 같은 패턴).
                self.widget.set_action_tone(
                    "hist.clear",
                    if armed {
                        nclip_ctl::controls::ButtonTone::Default
                    } else {
                        nclip_ctl::controls::ButtonTone::Danger
                    },
                    &mut inv,
                );
                if armed {
                    self.clear_arm = None;
                    self.clear_request = true;
                    self.widget.set_row_note_toned(
                        "hist.clear",
                        nclip_core::tr(lang, nclip_core::Msg::NoteClearDone),
                        nclip_ui::NoteTone::Ok,
                        &mut inv,
                    );
                } else {
                    self.clear_arm = Some(Instant::now());
                    self.widget.set_row_note_toned(
                        "hist.clear",
                        nclip_core::tr(lang, nclip_core::Msg::NoteClearArmed),
                        nclip_ui::NoteTone::Warn,
                        &mut inv,
                    );
                }
                self.redraw();
            }
            // ★ 연결 해제(09-03) — **즉시** 끊고 Connected 자리(sync.test)에 표시.
            if key == "sync.disconnect" && val == "run" {
                self.sync_drop_now(); // 버튼은 연결 중에만 활성 — 별도 안내 노트 없음(09-03).
            }
            // ★ Dock 아이콘(T-12e mac · 09-03 사용자) — 끔 = Accessory(메뉴 막대 전용).
            //   즉시 반영 · 다음 시작은 셸이 기동 때 적용. 다른 OS no-op.
            if key == "ui.dock_icon" {
                nclip_plat::dock::set_dock_visible(val == "on");
            }
            if key == "app.autostart" {
                let on = val == "on";
                match nclip_plat::autostart::apply(on) {
                    Ok(()) => {
                        self.conf.set(
                            "app.autostart_reg",
                            if on { "on" } else { "off" }.into(),
                            now,
                        );
                        println!(
                            "자동 시작: {}",
                            if on {
                                "등록됨 (로그인 시 실행)"
                            } else {
                                "해제됨"
                            }
                        );
                    }
                    Err(e) => eprintln!("자동 시작 등록 실패: {e} — 값은 유지됩니다(재시도 가능)"),
                }
            }
        }
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
        let (iw, ih) = (size.width as i32, size.height as i32);
        if self.laid_out != (iw, ih) {
            let mut inv = Invalidations::default();
            self.widget.set_bounds(Rect::new(0, 0, iw, ih), &mut inv);
            self.laid_out = (iw, ih);
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        {
            let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
            let mut dc = // ★ 배율은 위젯 레이아웃과 **같은 값**이어야 한다 — 다르면 Retina에서
            //   글자만 작아진다(08-27 macOS 회귀).
            RasterCtx::new(&mut gfx, &self.font, self.scale);
            dc.fill_rect(Rect::new(0, 0, iw, ih), self.theme.window_bg);
            self.widget.paint(&mut dc, &self.theme);
            if let Some(c) = self.capture.as_mut() {
                paint_capture(c, &mut dc, iw, ih, &self.theme, self.scale);
            }
        }
        let _ = buf.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        // ★ 단독 `settings` 명령의 진입 — 상주 셸은 resumed를 위임하지 않고
        //   트레이 "열기"에서 [`Self::ensure_window`]를 부른다(시작은 트레이만).
        if !self.resident {
            self.ensure_window(el);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // ★ 캡처 오버레이가 열려 있으면 키·마우스는 오버레이가 전부 먹는다(모달 · 09-04).
        if self.capture.is_some() {
            match &event {
                WindowEvent::ModifiersChanged(m) => {
                    self.mods = m.state();
                    return;
                }
                WindowEvent::KeyboardInput { event: kev, .. } => {
                    let kev = kev.clone();
                    self.capture_key(el, &kev);
                    return;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor = (position.x as i32, position.y as i32);
                    let (x, y) = self.cursor;
                    self.capture_mouse(InputEvent::MouseMove { x, y });
                    return;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if *button == winit::event::MouseButton::Left {
                        let (x, y) = self.cursor;
                        let ev = if *state == ElementState::Pressed {
                            InputEvent::MouseDown {
                                x,
                                y,
                                shift: false,
                                primary: false,
                            }
                        } else {
                            InputEvent::MouseUp { x, y }
                        };
                        self.capture_mouse(ev);
                    }
                    return;
                }
                WindowEvent::MouseWheel { .. } => return,
                _ => {}
            }
        }
        match event {
            WindowEvent::CloseRequested => self.request_close(el),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                let mut inv = Invalidations::default();
                self.widget.set_scale(self.scale, &mut inv);
                self.laid_out = (0, 0); // 재배치 강제
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.paint(),
            // Windows/mac은 winit이 OS 테마 변경을 창 이벤트로 준다(Linux는 포털 신호 — tray 셸).
            WindowEvent::ThemeChanged(_) => self.apply_theme(),

            WindowEvent::MouseInput { state, button, .. } => {
                let (x, y) = self.cursor;
                // ★ 우클릭 = 텍스트 입력 편집 메뉴(09-03) — 위젯이 포커스된 입력 안일 때만 연다.
                if button == winit::event::MouseButton::Right {
                    if state == ElementState::Pressed {
                        self.widget
                            .set_clipboard_has_text(crate::cliptext::has_text());
                        self.feed(InputEvent::RightDown { x, y });
                    }
                    return;
                }
                if button != winit::event::MouseButton::Left {
                    return;
                }
                let ev = if state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        x,
                        y,
                        shift: self.mods.shift_key(),
                        primary: self.primary(),
                    }
                } else {
                    InputEvent::MouseUp { x, y }
                };
                self.feed(ev);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                let (x, y) = self.cursor;
                self.feed(InputEvent::MouseMove { x, y });
                // ★ 스플리터 위면 좌우 리사이즈 커서로 바꾼다(VS Code 방식 —
                //   위젯은 "보여야 하는가"만 말하고, OS 커서 번역은 호스트 몫이다).
                let want = self.widget.wants_col_resize_cursor(x, y);
                if want != self.col_resize {
                    self.col_resize = want;
                    if let Some(w) = &self.window {
                        w.set_cursor(if want {
                            CursorIcon::ColResize
                        } else {
                            CursorIcon::Default
                        });
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // 원시 delta를 그대로 넘긴다 — 분수 노치 누적은 위젯이 `WheelAccum`으로 한다.
                let raw = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * WHEEL_DELTA as f32) as i32,
                    MouseScrollDelta::PixelDelta(p) => (p.y as f32 * 4.0) as i32,
                };
                if raw != 0 {
                    self.feed(InputEvent::Wheel { delta: raw });
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let shift = self.mods.shift_key();
                let primary = self.primary();
                // ★ 편집 단축키는 **물리 키**로 판정(09-04 실기 — 한글 자판에서 Ctrl+V의 논리 키가 'ㅍ'이라 붙여넣기가 안 먹었다).
                let named = |k: CtlKey| InputEvent::Key {
                    key: k,
                    shift,
                    primary,
                };
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => self.request_close(el),
                    Key::Named(NamedKey::ArrowUp) => self.feed(named(CtlKey::Up)),
                    Key::Named(NamedKey::ArrowDown) => self.feed(named(CtlKey::Down)),
                    Key::Named(NamedKey::ArrowLeft) => self.feed(named(CtlKey::Left)),
                    Key::Named(NamedKey::ArrowRight) => self.feed(named(CtlKey::Right)),
                    Key::Named(NamedKey::PageUp) => self.feed(named(CtlKey::PageUp)),
                    Key::Named(NamedKey::PageDown) => self.feed(named(CtlKey::PageDown)),
                    Key::Named(NamedKey::Home) => self.feed(named(CtlKey::Home)),
                    Key::Named(NamedKey::End) => self.feed(named(CtlKey::End)),
                    Key::Named(NamedKey::Enter) => self.feed(named(CtlKey::Enter)),
                    Key::Named(NamedKey::Space) => self.feed(named(CtlKey::Space)),
                    Key::Named(NamedKey::Delete) => self.feed(named(CtlKey::Delete)),
                    // Backspace는 Char('\u{8}')로 온다(접두사 축소 — 검색창 계약).
                    Key::Named(NamedKey::Backspace) => {
                        let now = self.now_ms();
                        self.feed(InputEvent::Char {
                            c: '\u{8}',
                            now_ms: now,
                        });
                    }
                    _ if primary
                        && phys_is(&event.physical_key, winit::keyboard::KeyCode::KeyA) =>
                    {
                        self.feed(InputEvent::SelectAll);
                    }
                    // ★ 클립보드 단축(09-03 — 암호·핸들 입력란): 복사·잘라내기·붙여넣기.
                    _ if primary
                        && phys_is(&event.physical_key, winit::keyboard::KeyCode::KeyC) =>
                    {
                        if let Some(s) = self.widget.clipboard_copy() {
                            crate::cliptext::set_text(&s);
                        }
                    }
                    _ if primary
                        && phys_is(&event.physical_key, winit::keyboard::KeyCode::KeyX) =>
                    {
                        let mut inv = Invalidations::default();
                        if let Some(s) = self.widget.clipboard_cut(&mut inv) {
                            crate::cliptext::set_text(&s);
                        }
                        self.drain_changes();
                        self.redraw();
                    }
                    _ if primary
                        && phys_is(&event.physical_key, winit::keyboard::KeyCode::KeyV) =>
                    {
                        if let Some(s) = crate::cliptext::get_text() {
                            let mut inv = Invalidations::default();
                            self.widget
                                .clipboard_paste(s.trim_end_matches('\n'), &mut inv);
                            self.drain_changes();
                            self.redraw();
                        }
                    }
                    _ => {
                        if let Some(txt) = event.text.as_ref() {
                            let now = self.now_ms();
                            for c in txt.chars().filter(|c| !c.is_control()) {
                                self.feed(InputEvent::Char { c, now_ms: now });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // 캐럿 깜빡임·툴팁·★ 상태 페이드 — 위젯이 "다시 그려야 한다"고 할 때만.
        let now = self.now_ms();
        self.fire_lan_respawn(); // ★ 릴레이 None 자동 재기동(09-05) — 디바운스 만료 시 셸에 요청.
        self.poll_sync_test(); // ★ 동기화 테스트 결과 소비(09-03).
        self.poll_sync_status(); // ★ 러너 상태 → 노트(자동 Test 표시).
        let animating = self.widget.tick(now);
        if animating {
            self.redraw();
        }
        // ★ 저장 판정 — 값 변경 때가 아니라 **여기서** 한다(연속 변경을 한 번으로 합친다).
        let wall = Instant::now();
        if self.conf.tick(wall) {
            println!("설정 저장: {}", self.conf.path().display());
        }
        // 애니메이션 중이거나 저장을 기다리는 중이면 다음 깨어남을 예약한다
        // (Wait만 두면 이벤트가 없어 타이머가 영영 안 돈다).
        let next = if animating {
            Some(Duration::from_millis(16))
        } else if self.conf.dirty() {
            Some(Duration::from_millis(250))
        } else {
            None
        };
        // ★ 자동 재기동 예약이 있으면 그 만료에도 깨어난다(Wait만 두면 타이머가 영영 안 돈다).
        let deadline = match (self.sync_respawn_at, self.name_announce_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let next = match (next, deadline) {
            (Some(d), Some(at)) => Some(d.min(at.saturating_duration_since(wall))),
            (None, Some(at)) => Some(at.saturating_duration_since(wall)),
            (d, None) => d,
        };
        match next {
            Some(d) => el.set_control_flow(ControlFlow::WaitUntil(wall + d)),
            // 조용해지면 다시 이벤트 대기로 — 상주 앱이 유휴에서 CPU를 쓰면 안 된다.
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// 설정 창 실행.
pub(crate) fn run() {
    let conf_probe = Settings::load();
    crate::conf::apply_lang(&conf_probe);
    let Some(font) = crate::conf::load_ui_font(&conf_probe) else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다.");
        std::process::exit(1);
    };
    drop(conf_probe);
    // ★ 저장된 값을 먼저 읽는다 — 위젯은 이 값으로 시작해야 한다(기본값으로 그린 뒤
    //   덮어쓰면 첫 프레임이 잘못된 값으로 한 번 깜빡인다).
    let conf = Settings::load();
    println!(
        "폰트: {} · 설정 항목 {}개",
        nclip_plat::font::system_ui_font_name().unwrap_or("(이름 미상)"),
        nclip_ui::registry().len()
    );
    println!("설정 파일: {}", conf.path().display());
    println!("창을 엽니다 — 상단에서 설정 검색 · 값 변경은 콘솔에 찍힙니다 · Esc 종료");

    let Ok(el) = EventLoop::new() else {
        eprintln!("이벤트 루프 생성 실패");
        std::process::exit(1);
    };
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(font, conf, false);
    if let Err(e) = el.run_app(&mut app) {
        eprintln!("이벤트 루프 오류: {e}");
        std::process::exit(1);
    }
    // ★ 종료 직전 강제 수거 — 이게 없으면 "바꾸고 바로 닫으면 안 저장됨"이 된다.
    if app.conf.flush() {
        println!("설정 저장: {}", app.conf.path().display());
    }
}

/// 창에 앱 식별자를 싣는다 — Wayland `app_id` · X11 `WM_CLASS`(beep 08-29 실기 ③: 없으면
/// GNOME Dock이 `.desktop`과 못 맞춰 톱니바퀴 + "알 수 없음"). 다른 OS는 무해한 no-op.
/// ★ 창을 진짜 앞으로(09-01 사용자 실기 "트레이 클릭 시 창이 뒤로 숨음") —
/// Windows는 포그라운드 권한 규칙 때문에 `focus_window()`만으로는 작업표시줄만 깜밖이고
/// 말아서, K-1의 AttachThreadInput 문법(`nclip_plat::paste::force_foreground`)을 재사용한다.
pub(crate) fn bring_to_front(win: &winit::window::Window) {
    #[cfg(windows)]
    {
        use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
        if let Ok(h) = win.window_handle() {
            if let RawWindowHandle::Win32(w) = h.as_raw() {
                let _ = nclip_plat::paste::force_foreground(w.hwnd.get());
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // ★ Accessory(독 숨김)에서도 트레이 "열기"가 창을 진짜 앞으로(09-03) —
        //   창 포커스 전에 앱 활성화가 선행돼야 한다(Dock 아이콘 없이도 활성 전환).
        nclip_plat::dock::activate_front();
        win.focus_window();
    }
    // ★ Linux/X11(09-05) — winit `focus_window()`가 소스=1(앱)이라 Mutter에 막혀 창이 뒤에 깔리고
    //   "준비됨" 알림만 뜬다. 페이저 소스로 우리가 직접 올린다(메인창·설정 창 · 신규/재표시 공통 길목).
    //   Wayland 네이티브 창은 X 창 id가 없어 no-op(설정 창의 토큰 경로가 맡는다).
    #[cfg(target_os = "linux")]
    let _ = linux_raise_x11(win);
    let _ = win;
}

/// ★ Linux/X11 창 올리기(09-05) — winit 창 핸들에서 X 창 id를 뽑아 페이저 소스로 활성화한다.
///   Wayland 네이티브 핸들·핸들 부재 = false(호출측이 토큰/주의요청으로 폴백).
#[cfg(target_os = "linux")]
pub(crate) fn linux_raise_x11(win: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    let Ok(h) = win.window_handle() else {
        return false;
    };
    let xid = match h.as_raw() {
        RawWindowHandle::Xlib(x) => x.window as u32,
        RawWindowHandle::Xcb(x) => x.window.get(),
        _ => return false, // Wayland 네이티브 — 토큰 경로가 맡는다
    };
    nclip_plat::paste::raise_x11_window(xid)
}

pub(crate) fn win_name(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    // ★ 프로필 실행(09-04)은 창 제목 끝에 `[프로필]` — 두 인스턴스를 눈으로 가른다.
    let attrs = match crate::conf::profile() {
        Some(p) => {
            let t = format!("{} [{p}]", attrs.title);
            attrs.with_title(t)
        }
        None => attrs,
    };
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::WindowAttributesExtWayland;
        use winit::platform::x11::WindowAttributesExtX11;
        WindowAttributesExtWayland::with_name(
            WindowAttributesExtX11::with_name(attrs, "nexa-clip", "nexa-clip"),
            "nexa-clip",
            "nexa-clip",
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        attrs
    }
}

/// Wayland 정식 활성화 — winit 창의 raw 핸들(wl_display·wl_surface)로
/// `nclip_plat::wlactivate::activate`. X11·핸들 부재 = false.
#[cfg(target_os = "linux")]
fn wayland_activate(w: &Window, token: &str) -> bool {
    use winit::raw_window_handle::{
        HasDisplayHandle as _, HasWindowHandle as _, RawDisplayHandle, RawWindowHandle,
    };
    let (Ok(d), Ok(s)) = (w.display_handle(), w.window_handle()) else {
        return false;
    };
    match (d.as_raw(), s.as_raw()) {
        (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(s)) => {
            // SAFETY: 살아 있는 winit 창의 핸들 · 메인(이벤트 루프) 스레드에서 호출.
            unsafe {
                nclip_plat::wlactivate::activate(d.display.as_ptr(), s.surface.as_ptr(), token)
            }
        }
        _ => false,
    }
}

/// ★ 패스프레이즈 추천(09-03) — OS 난수 시드 해시(`RandomState`) 2회 → base32풍 12자
/// (xxxx-xxxx-xxxx). 계정 비밀번호가 아니라 **만남 지점 재료**(docs/09 §6-2)라 이 강도면
/// 스캔 방어 목적을 충족한다 — 더 강하게 쓰고 싶으면 직접 입력.
fn suggest_passphrase() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let mut bits = [0u64; 2];
    for b in &mut bits {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos().into()),
        );
        *b = h.finish();
    }
    const AL: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789"; // 혼동 글자(i·l·o·0·1) 제외
    let mut out = String::new();
    let mut v = u128::from(bits[0]) << 64 | u128::from(bits[1]);
    for i in 0..12 {
        if i > 0 && i % 4 == 0 {
            out.push('-');
        }
        out.push(AL[(v % AL.len() as u128) as usize] as char);
        v /= AL.len() as u128;
    }
    out
}

/// 릴레이 1회 접속(테스트) — 성공 = (주소, 서버 핀 앞 8자). 핀은 TOFU로 고정한다.
fn sync_test_once(raw: &str, dir: &std::path::Path) -> Result<(String, String), String> {
    let (addr_str, addr) =
        nclip_sync::relay::resolve_server(raw).ok_or_else(|| format!("주소 해석 실패: {raw}"))?;
    let (id, _) = nclip_sync::keyfile::load_or_generate(&dir.join("identity.key"))
        .map_err(|e| format!("신원 키: {e}"))?;
    let rids = nclip_sync::relay::rids_around(&id.peer_id());
    let pin_path = dir.join("server.pin");
    let expected = nclip_sync::relay::pinfile::lookup(&pin_path, &addr_str);
    let client = nclip_sync::relay::RelayClient::connect(addr, &id, &rids, expected)
        .map_err(|e| format!("{e:?}"))?;
    let pin = nclip_sync::relay::peer_hex(&client.server_peer());
    if expected.is_none() {
        let _ = nclip_sync::relay::pinfile::store(&pin_path, &addr_str, &client.server_peer());
    }
    Ok((addr_str, pin[..8].to_string()))
}

/// 설정 창 첫 배치 — (물리 좌표, 물리 크기, 모니터 키).
type Placement = ((i32, i32), (u32, u32), String);

/// 모니터 식별 키(09-04) — 이름이 있으면 이름, 없으면 원점+크기. 설정 창 마지막 위치를 "같은 모니터일 때만" 복원하는 열쇠.
fn monitor_key(m: &winit::monitor::MonitorHandle) -> String {
    let p = m.position();
    let s = m.size();
    match m.name() {
        Some(n) if !n.is_empty() => format!("{n}@{}x{}", s.width, s.height),
        _ => format!("{}:{}@{}x{}", p.x, p.y, s.width, s.height),
    }
}

/// winit 물리 키 → 단축키 토큰(09-04 캡처) — 글자·숫자·F키·편집/이동 키만. 수정 키 단독·그 밖은 `None`.
fn keycode_token(pk: &winit::keyboard::PhysicalKey) -> Option<&'static str> {
    use winit::keyboard::{KeyCode as K, PhysicalKey};
    let PhysicalKey::Code(code) = pk else {
        return None;
    };
    Some(match code {
        K::KeyA => "A",
        K::KeyB => "B",
        K::KeyC => "C",
        K::KeyD => "D",
        K::KeyE => "E",
        K::KeyF => "F",
        K::KeyG => "G",
        K::KeyH => "H",
        K::KeyI => "I",
        K::KeyJ => "J",
        K::KeyK => "K",
        K::KeyL => "L",
        K::KeyM => "M",
        K::KeyN => "N",
        K::KeyO => "O",
        K::KeyP => "P",
        K::KeyQ => "Q",
        K::KeyR => "R",
        K::KeyS => "S",
        K::KeyT => "T",
        K::KeyU => "U",
        K::KeyV => "V",
        K::KeyW => "W",
        K::KeyX => "X",
        K::KeyY => "Y",
        K::KeyZ => "Z",
        K::Digit0 => "0",
        K::Digit1 => "1",
        K::Digit2 => "2",
        K::Digit3 => "3",
        K::Digit4 => "4",
        K::Digit5 => "5",
        K::Digit6 => "6",
        K::Digit7 => "7",
        K::Digit8 => "8",
        K::Digit9 => "9",
        K::F1 => "F1",
        K::F2 => "F2",
        K::F3 => "F3",
        K::F4 => "F4",
        K::F5 => "F5",
        K::F6 => "F6",
        K::F7 => "F7",
        K::F8 => "F8",
        K::F9 => "F9",
        K::F10 => "F10",
        K::F11 => "F11",
        K::F12 => "F12",
        K::Space => "Space",
        K::Tab => "Tab",
        K::Insert => "Insert",
        K::Delete => "Delete",
        K::Home => "Home",
        K::End => "End",
        K::PageUp => "PageUp",
        K::PageDown => "PageDown",
        K::ArrowUp => "Up",
        K::ArrowDown => "Down",
        K::ArrowLeft => "Left",
        K::ArrowRight => "Right",
        _ => return None,
    })
}

/// 물리 키 비교(09-04) — 한글/다른 자판에서도 Ctrl+C/V/X/A가 같은 자리로 잡힌다.
fn phys_is(pk: &winit::keyboard::PhysicalKey, code: winit::keyboard::KeyCode) -> bool {
    matches!(pk, winit::keyboard::PhysicalKey::Code(c) if *c == code)
}

/// ★ 릴레이 None 자동 재기동 디바운스(09-05) — 자유 문자열 행은 글자마다 오므로 마지막 입력 뒤 이만큼 잠잠해야 건다.
const LAN_RESPAWN_DEBOUNCE: Duration = Duration::from_millis(800);
/// ★ 이름 재소개 디바운스(09-05) — nexa-conf SaveScheduler의 quiet(1s)와 같은 박자 = "저장 시점".
const NAME_ANNOUNCE_DEBOUNCE: Duration = Duration::from_millis(1000);

/// 릴레이 None 자동 재기동 조건(순수 판정) — 동기화 켜짐 · 릴레이 `none` · 핸들·암호 둘 다 있음.
///   서버 릴레이는 종전 계약(정보 변경 = 해제 → Test로 재접속)을 유지한다.
fn lan_auto_respawn_ready(relay: &str, enabled: &str, handle: &str, pass: &str) -> bool {
    relay.trim() == "none"
        && enabled == "on"
        && !handle.trim().is_empty()
        && !pass.trim().is_empty()
}

#[cfg(test)]
mod lan_respawn_tests {
    use super::lan_auto_respawn_ready;

    #[test]
    fn ready_only_when_none_enabled_and_identity_full() {
        assert!(lan_auto_respawn_ready("none", "on", "kiros33", "pw"));
        assert!(
            lan_auto_respawn_ready(" none ", "on", " kiros33 ", "pw"),
            "공백 무시"
        );
        assert!(
            !lan_auto_respawn_ready("none", "on", "", "pw"),
            "핸들 비면 안 건다"
        );
        assert!(
            !lan_auto_respawn_ready("none", "on", "kiros33", "  "),
            "암호 비면 안 건다"
        );
        assert!(
            !lan_auto_respawn_ready("none", "off", "kiros33", "pw"),
            "동기화 꺼짐"
        );
        assert!(
            !lan_auto_respawn_ready("beepd.sosomlab.com", "on", "kiros33", "pw"),
            "서버 릴레이는 종전 계약(Test) 유지"
        );
        assert!(
            !lan_auto_respawn_ready("", "on", "kiros33", "pw"),
            "빈 릴레이 = 기본 서버"
        );
    }
}
