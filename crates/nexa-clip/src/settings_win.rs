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

use nclip_ctl::draw::DrawCtx;
use nclip_ctl::event::{InputEvent, Key as CtlKey, WHEEL_DELTA};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
use nclip_ctl::widget::{Invalidations, Widget};
use nclip_gfx::{Font, Surface};
use nclip_ui::{SettingsState, SettingsWidget};

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    scale: f32,
    state: SettingsState,
    widget: SettingsWidget,
    mods: ModifiersState,
    started: Instant,
    /// 마지막으로 준 크기 — 바뀔 때만 `set_bounds`를 부른다.
    laid_out: (i32, i32),
    /// 마지막 커서 위치(winit은 클릭 이벤트에 좌표를 싣지 않는다).
    cursor: (i32, i32),
}

impl App {
    fn new(font: Font) -> Self {
        let state = SettingsState::with_defaults();
        let widget = SettingsWidget::new(&state);
        Self {
            window: None,
            ctx: None,
            surface: None,
            font,
            theme: Theme::dark(),
            scale: 1.0,
            state,
            widget,
            mods: ModifiersState::empty(),
            started: Instant::now(),
            laid_out: (0, 0),
            cursor: (0, 0),
        }
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

    fn drain_changes(&mut self) {
        for (key, val) in self.widget.take_changes() {
            // ★ 즉시 적용 계약 — 영속(nexa-conf)은 T-12c2에서 붙인다.
            self.state.set(key, val.clone());
            println!("설정 변경: {key} = {val}");
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
            let mut dc = RasterCtx::new(&mut gfx, &self.font);
            dc.fill_rect(Rect::new(0, 0, iw, ih), self.theme.window_bg);
            self.widget.paint(&mut dc, &self.theme);
        }
        let _ = buf.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Nexa Clip — 설정 (검색 · T 테마 · Esc 종료)")
            .with_inner_size(winit::dpi::LogicalSize::new(760.0, 560.0));
        let Ok(win) = el.create_window(attrs) else {
            eprintln!("창 생성 실패");
            el.exit();
            return;
        };
        let win = Rc::new(win);
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
        self.window = Some(win);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                let mut inv = Invalidations::default();
                self.widget.set_scale(self.scale, &mut inv);
                self.laid_out = (0, 0); // 재배치 강제
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.paint(),

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
                    Key::Named(NamedKey::Escape) => el.exit(),
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

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        // 캐럿 깜빡임·툴팁 타이머 — 위젯이 다시 그려야 한다고 하면 그때만.
        let now = self.now_ms();
        if self.widget.tick(now) {
            self.redraw();
        }
    }
}

/// 설정 창 실행.
pub(crate) fn run() {
    let Some((data, idx)) = nclip_plat::font::system_ui_font() else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다.");
        std::process::exit(1);
    };
    let font = match Font::from_static(data, idx) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("폰트 로드 실패: {e:?}");
            std::process::exit(1);
        }
    };
    println!(
        "폰트: {} · 설정 항목 {}개",
        nclip_plat::font::system_ui_font_name().unwrap_or("(이름 미상)"),
        nclip_ui::registry().len()
    );
    println!("창을 엽니다 — 상단에서 설정 검색 · 값 변경은 콘솔에 찍힙니다 · Esc 종료");

    let Ok(el) = EventLoop::new() else {
        eprintln!("이벤트 루프 생성 실패");
        std::process::exit(1);
    };
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(font);
    if let Err(e) = el.run_app(&mut app) {
        eprintln!("이벤트 루프 오류: {e}");
        std::process::exit(1);
    }
}
