//! `demo` — ★ **렌더 파이프라인 실증**(T-12b2).
//!
//! 창을 열고 [`nclip_gfx`] CPU 래스터라이저 위에 **S1 퀵 팝업 레이아웃**을 그린다.
//! "hello world" 대신 **실제 설계**를 그리는 이유는, 이 단계에서 확인해야 할 것이
//! *"픽셀이 나오는가"* 만이 아니라 ***"우리가 정한 화면이 실제로 그렇게 보이는가"*** 이기 때문이다.
//!
//! | 검증 대상 | 어떻게 |
//! |---|---|
//! | 렌더 파이프라인 | winit 창 → softbuffer 버퍼 → `Surface` → `RasterCtx` |
//! | 시스템 폰트 + 한글 | `nclip_plat::font` mmap → `Font::from_static` |
//! | ★ **보기 3모드** | `1`·`2`·`3` 키로 전환 — 행 높이가 실제로 달라지는가 |
//! | ★ **세로 밀도**(DR-14) | 크롬 1~2줄 · 같은 높이에서 항목이 몇 개 보이는가 |
//! | 테마 | `T` 키로 다크/라이트 — 색 하드코딩이 없는가 |
//! | ★ **알파 합성** | `P` 키로 **반투명 미리보기 패널** — 뒤 목록이 비치는가([docs/23](../../../docs/23-alpha-rendering.md)) |

use nclip_ctl::draw::{DrawCtx, FontSlot};
use nclip_ctl::geom::Rect;
use nclip_ctl::raster::RasterCtx;
use nclip_ctl::theme::Theme;
use nclip_ctl::ViewMode;
use nclip_gfx::{Font, Surface};

use std::num::NonZeroU32;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// 데모 목록 행 — 실제 [`nclip_core::ClipItem`]을 흉내 낸다.
struct Row {
    icon: &'static str,
    text: &'static str,
    app: &'static str,
    lines: u8, // 일반 보기에서 차지할 줄 수(이미지·서식 흉내)
}

const ROWS: &[Row] = &[
    Row {
        icon: "▣",
        text: "스크린샷 1920×1080",
        app: "Chrome",
        lines: 4,
    },
    Row {
        icon: "📄",
        text: "CREATE INDEX BISCM.IX_M4…ITEMOP');↵",
        app: "DBeaver",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "내용 정리 후 진행사항 최신화…main 병합한 뒤",
        app: "Claude",
        lines: 2,
    },
    Row {
        icon: "📁",
        text: "3개 파일 — 보고서.xlsx 외 2",
        app: "탐색기",
        lines: 3,
    },
    Row {
        icon: "🎨",
        text: "#2D6A4F",
        app: "Figma",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "https://github.com/SosomLab",
        app: "Chrome",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "nexa-beep 클립보드 \"왕복\" 시험",
        app: "Terminal",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "M4S_I003081",
        app: "DBeaver",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "START_YYMMDD",
        app: "DBeaver",
        lines: 1,
    },
    Row {
        icon: "▣",
        text: "PPT 슬라이드 캡처",
        app: "PowerPoint",
        lines: 4,
    },
    Row {
        icon: "📄",
        text: "0.2.6 버전에서 0.2.8 버전으로 brew…",
        app: "Terminal",
        lines: 1,
    },
    Row {
        icon: "📄",
        text: "$mp = \"$env:ProgramFiles\\Wind…xe\"",
        app: "PowerShell",
        lines: 1,
    },
];

struct App {
    window: Option<Rc<Window>>,
    ctx: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    font: Font,
    theme: Theme,
    mode: ViewMode,
    scale: f32,
    /// ★ 반투명 미리보기 패널(알파 합성 실증).
    preview: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Nexa Clip — 렌더 데모 (1/2/3 보기 · P 미리보기 · T 테마 · Esc)")
            .with_inner_size(winit::dpi::LogicalSize::new(560.0, 620.0));
        let Ok(win) = el.create_window(attrs) else {
            eprintln!("창 생성 실패");
            el.exit();
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
        self.window = Some(win);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => el.exit(),
                    Key::Character("1") => self.set_mode(ViewMode::Rich),
                    Key::Character("2") => self.set_mode(ViewMode::Compact),
                    Key::Character("3") => self.set_mode(ViewMode::Plain),
                    Key::Character("p" | "P") => {
                        self.preview = !self.preview;
                        self.request_redraw();
                    }
                    Key::Character("t" | "T") => {
                        self.theme = if self.theme.is_dark {
                            Theme::light()
                        } else {
                            Theme::dark()
                        };
                        self.request_redraw();
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => self.paint(),
            _ => {}
        }
    }
}

impl App {
    fn set_mode(&mut self, m: ViewMode) {
        if self.mode != m {
            self.mode = m;
            self.request_redraw();
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
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
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };

        {
            let mut gfx = Surface::new(&mut buf, size.width as usize, size.height as usize);
            let mut dc = // ★ 배율은 위젯 레이아웃과 **같은 값**이어야 한다 — 다르면 Retina에서
            //   글자만 작아진다(08-27 macOS 회귀).
            RasterCtx::new(&mut gfx, &self.font, self.scale);
            draw_popup(
                &mut dc,
                self.theme,
                self.mode,
                self.scale,
                size.width as i32,
                size.height as i32,
                self.preview,
            );
        }

        let _ = buf.present();
    }
}

/// S1 퀵 팝업을 그린다 — [docs/04 §2-1](../../../docs/04-feature-scope-and-screens.md) 레이아웃.
#[allow(clippy::too_many_arguments)]
fn draw_popup(
    dc: &mut RasterCtx<'_, '_, '_>,
    th: Theme,
    mode: ViewMode,
    s: f32,
    w: i32,
    h: i32,
    preview: bool,
) {
    let px = |v: f32| (v * s).round() as i32;
    let full = Rect::new(0, 0, w, h);
    dc.select_font(FontSlot::Base, false);

    let header_h = px(38.0);
    let footer_h = px(28.0);
    let pad = px(10.0);

    // 배경
    dc.fill_rect(full, th.window_bg);

    // ── ① 헤더 1줄: 이름 + 검색 필드 + 미리보기 토글 ──
    dc.fill_rect(Rect::new(0, 0, w, header_h), th.chrome_bg);
    dc.text(pad, px(11.0), full, "Nexa Clip", th.text_dim);
    let name_w = dc.text_width("Nexa Clip");
    let field_x = pad + name_w + px(10.0);
    let field_w = w - field_x - pad - px(30.0);
    dc.fill_round_rect(
        Rect::new(field_x, px(7.0), field_w, px(24.0)),
        px(6.0),
        th.field_bg,
    );
    dc.text(
        field_x + px(8.0),
        px(11.0),
        full,
        "검색어를 입력하세요…",
        th.text_dim,
    );
    dc.fill_round_rect(
        Rect::new(w - pad - px(24.0), px(7.0), px(24.0), px(24.0)),
        px(6.0),
        th.field_bg,
    );
    dc.text(w - pad - px(17.0), px(11.0), full, "▤", th.text_dim);
    dc.fill_rect(Rect::new(0, header_h - 1, w, 1), th.border);

    // ── ② 목록 ──
    let list_top = header_h;
    let list_bot = h - footer_h;
    let mut y = list_top;
    let line_h = px(17.0);
    for (i, row) in ROWS.iter().enumerate() {
        let row_h = match mode {
            ViewMode::Plain => px(24.0),
            ViewMode::Compact => px(34.0),
            // ★ 일반 보기만 행 높이가 내용에 따라 다르다 — 가변 높이 가상화가 필요한 이유.
            ViewMode::Rich => px(30.0) + line_h * i32::from(row.lines),
        };
        if y >= list_bot {
            break;
        }
        let clip = Rect::new(0, y, w, (list_bot - y).min(row_h));
        let selected = i == 0;
        if selected {
            dc.fill_rect(clip, th.sel_bg);
        } else if i % 2 == 1 && mode != ViewMode::Plain {
            dc.fill_rect(clip, th.panel_bg_alt);
        }

        match mode {
            // 한 줄 보기 — 평문 1줄만. 최대 밀도.
            ViewMode::Plain => {
                dc.text(pad, y + px(4.0), clip, row.text, th.text);
            }
            // 간략 보기 — 아이콘 + 요약 + 출처 (기본값)
            ViewMode::Compact => {
                dc.text(pad, y + px(9.0), clip, row.icon, th.accent);
                dc.text(pad + px(20.0), y + px(9.0), clip, row.text, th.text);
                let aw = dc.text_width(row.app);
                dc.text(w - pad - aw, y + px(9.0), clip, row.app, th.text_dim);
            }
            // 일반 보기 — 내용 미리보기를 실제로 펼친다.
            ViewMode::Rich => {
                dc.text(pad, y + px(7.0), clip, row.icon, th.accent);
                dc.text(pad + px(20.0), y + px(7.0), clip, row.text, th.text);
                let aw = dc.text_width(row.app);
                dc.text(w - pad - aw, y + px(7.0), clip, row.app, th.text_dim);
                let body = Rect::new(
                    pad + px(20.0),
                    y + px(26.0),
                    w - pad * 2 - px(20.0),
                    line_h * i32::from(row.lines) - px(4.0),
                );
                dc.fill_round_rect(body, px(4.0), th.bubble_peer);
            }
        }
        // 번호 단축키 — ★ 맨숫자가 아니라 수식 키 조합(검색 필드가 포커스를 갖는다).
        if i < 9 && mode != ViewMode::Plain {
            let k = format!("^{}", i + 1);
            let kw = dc.text_width(&k);
            dc.text(w - pad - kw, y + row_h - px(14.0), clip, &k, th.text_dim);
        }
        y += row_h;
    }

    // ── ★ 반투명 미리보기 패널(알파 합성 실증 — docs/23 L1) ──
    if preview {
        let pw = (w * 45 / 100).max(px(180.0));
        let panel = Rect::new(
            w - pw - pad,
            list_top + pad,
            pw,
            list_bot - list_top - pad * 2,
        );
        // 그림자 — 낮은 알파로 두 겹(가장자리가 부드러워진다)
        for (off, a) in [(px(6.0), 0.10), (px(3.0), 0.16)] {
            dc.fill_rect_alpha(
                Rect::new(panel.x + off, panel.y + off, panel.w, panel.h),
                th.window_bg,
                a,
            );
        }
        // ★ 패널 본체 — 0.92 라 뒤 목록이 살짝 비친다
        dc.fill_rect_alpha(panel, th.panel_bg, 0.92);
        dc.fill_rect(Rect::new(panel.x, panel.y, panel.w, 1), th.border);
        dc.text(
            panel.x + px(10.0),
            panel.y + px(8.0),
            panel,
            "미리보기 (반투명 0.92)",
            th.text,
        );
        dc.text(
            panel.x + px(10.0),
            panel.y + px(28.0),
            panel,
            "뒤 목록이 비칩니다",
            th.text_dim,
        );
        // 구분선 — 색을 새로 만들지 않고 불투명도로(docs/23 A-5)
        dc.fill_rect_alpha(
            Rect::new(
                panel.x + px(10.0),
                panel.y + px(48.0),
                panel.w - px(20.0),
                1,
            ),
            th.text,
            0.25,
        );
    }

    // ── ③ 푸터 1줄 ── (Maccy는 4줄 · 우리는 1줄 — 세로 밀도 DR-14)
    let fy = h - footer_h;
    dc.fill_rect(Rect::new(0, fy, w, footer_h), th.chrome_bg);
    dc.fill_rect(Rect::new(0, fy, w, 1), th.border);
    let mode_label = match mode {
        ViewMode::Rich => "일반",
        ViewMode::Compact => "간략",
        ViewMode::Plain => "한 줄",
    };
    dc.text(
        pad,
        fy + px(6.0),
        full,
        &format!("{}개 · 🔒 · 보기: {mode_label}", ROWS.len()),
        th.text_dim,
    );
    let hint = "1/2/3 보기 · P 미리보기 · T 테마 · Esc";
    let hw = dc.text_width(hint);
    dc.text(w - pad - hw, fy + px(6.0), full, hint, th.text_dim);
}

/// 데모 실행.
pub(crate) fn run() {
    let Some((data, idx)) = nclip_plat::font::system_ui_font() else {
        eprintln!("시스템 UI 폰트를 찾지 못했습니다 — nclip_plat::font 후보 목록을 확인하세요.");
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
        "폰트: {}",
        nclip_plat::font::system_ui_font_name().unwrap_or("(이름 미상)")
    );
    println!("창을 엽니다 — 1/2/3 보기 · P 반투명 미리보기 · T 테마 · Esc 종료");

    let Ok(el) = EventLoop::new() else {
        eprintln!("이벤트 루프 생성 실패");
        std::process::exit(1);
    };
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        window: None,
        ctx: None,
        surface: None,
        font,
        theme: Theme::dark(),
        mode: ViewMode::default(),
        scale: 1.0,
        preview: false,
    };
    if let Err(e) = el.run_app(&mut app) {
        eprintln!("이벤트 루프 오류: {e}");
        std::process::exit(1);
    }
}
