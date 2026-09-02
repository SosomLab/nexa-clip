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

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

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
}

impl App {
    pub(crate) fn new(font: Font, conf: Settings, resident: bool) -> Self {
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
            mods: ModifiersState::empty(),
            started: Instant::now(),
            laid_out: (0, 0),
            cursor: (0, 0),
            col_resize: false,
            ui_refresh: false,
            resident,
        }
    }

    /// UI 전역 변경(언어 등) 1회성 수거 — 셸이 트레이/창 라벨을 새 언어로.
    pub(crate) fn take_ui_refresh(&mut self) -> bool {
        std::mem::take(&mut self.ui_refresh)
    }

    /// 창이 없으면 만들고, 있으면 앞으로 가져온다(트레이 "열기"의 재진입 경로).
    pub(crate) fn ensure_window(&mut self, el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.set_visible(true);
            // ★ Wayland는 `set_visible`/`focus_window`가 no-op(xdg-shell — 클라이언트는 창을
            //   되살릴 수 없다). 셸이 트레이 클릭 때 준 활성화 토큰이 있으면 **진짜 포커스**,
            //   없으면 주의 요청(Dock 강조 → 사용자가 한 번 클릭). beep 08-29 실기 그대로.
            #[cfg(target_os = "linux")]
            {
                w.set_minimized(false);
                let activated = nclip_plat::tray::take_activation_token()
                    .is_some_and(|tok| wayland_activate(w, &tok));
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
            el.exit();
        }
    }

    /// 창을 걷는다(상주는 유지) — ★ 미저장 변경은 여기서 강제 수거한다.
    fn hide_window(&mut self) {
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
        self.drain_changes();
        if !inv.is_empty() {
            self.redraw();
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
            // ★ 즉시 적용 계약 — 값은 바로 반영하고, **파일 쓰기는 미룬다**
            //   (조용해진 뒤 1초 · 늦어도 10초 — [`crate::conf`]).
            self.conf.set(key, val.clone(), now);
            println!("설정 변경: {key} = {val}");
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
                if button != winit::event::MouseButton::Left {
                    return;
                }
                let (x, y) = self.cursor;
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
                    Key::Character(t) if primary && t.eq_ignore_ascii_case("a") => {
                        self.feed(InputEvent::SelectAll);
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
    let _ = win;
}

pub(crate) fn win_name(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
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
