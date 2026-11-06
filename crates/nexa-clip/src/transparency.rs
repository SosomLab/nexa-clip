//! ★ 창 투명도(11-06 · nexa-dir3 T-102 · NEW-013 · T-199 · GAP-039 이식) — **호출 지점 + 포커스 묶음 판정**(3-OS).
//!
//! 설정 4키(모양 · [docs/14 §3-7](../../../docs/14-settings-registry.md)):
//! - `ui.transparency` — 포커스 있을 때(%) · 메인창 · **설정 창 · 팝업 · 새로 만드는 창**도 이 값.
//! - `ui.transparency_blur` — 포커스 없을 때(%) · **메인창만**(보조 창을 쓰는 동안 그 창이 흐려지지 않게).
//! - `ui.dialog_keeps_active` — 자체 창(설정 · 팝업)에 포커스가 있어도 메인을 활성으로 볼지(on = 한 묶음 · GAP-039).
//! - `ui.blur_grace_ms` — 묶음이 포커스를 잃은 뒤 메인을 흐리기까지의 유예(메인 → 보조 창 전환은 "잃음 → 얻음" 두
//!   사건이라 그 사이 깜빡임을 넘긴다 · 고급).
//!
//! 적용 = [`nclip_plat::opacity`](Windows 레이어드 알파 · macOS `alphaValue` · X11 `_NET_WM_WINDOW_OPACITY` · Wayland
//! 네이티브 미지원 → 호출부는 조용히 넘긴다). 핸들은 여기서 winit `raw_window_handle`로 꺼낸다(GAP-042 `taskbar_guard`와 같은 분담).
//! 창을 만드는 곳(메인 · 팝업 · 설정)은 생성 직후 [`apply`]를 부른다 · 포커스 사건·설정 변경은 셸(`tray_cmd.rs`)이
//! [`FocusGroup`]으로 판정해 [`apply_pct`]를 부른다. dir3 `perf.boost` FORCES(투명도 0 강제)는 clip에 성능 향상 모드가 없어 생략.
//!
//! 출처 = `../nexa-dir3/crates/nexa-dir/src/platform/mod.rs`(창 투명도 절) · `app/settings.rs::apply_transparency` ·
//! `app/event_loop.rs`(`on_focus_change` · `on_aux_focus` · `focus_group_active` · `blur_tick`).

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};
use winit::window::Window;

use nclip_plat::opacity::OPACITY_MAX_PCT;

/// 지금 새 창에 적용할 투명도(%) — 셸이 설정 유효값(포커스 값)으로 갱신한다.
static OPACITY_PCT: AtomicU8 = AtomicU8::new(0);

/// 새 창용 투명도(%)를 기억한다 — 이후 [`apply`]가 쓴다.
pub(crate) fn set_pct(pct: i64) {
    OPACITY_PCT.store(pct.clamp(0, OPACITY_MAX_PCT) as u8, Ordering::Relaxed);
}

/// 기억된 투명도(%).
pub(crate) fn pct() -> u8 {
    OPACITY_PCT.load(Ordering::Relaxed)
}

/// 창에 기억된(포커스) 투명도를 적용한다 — 창 생성 직후 · 보조 창.
pub(crate) fn apply(win: &Window) -> Result<(), &'static str> {
    apply_pct(win, i64::from(pct()))
}

/// 창에 **지정한** 투명도(%)를 적용한다 — 메인창의 포커스별 값. 실패 = 미지원 백엔드(Wayland) 또는 OS 호출 실패.
pub(crate) fn apply_pct(win: &Window, pct: i64) -> Result<(), &'static str> {
    use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    let alpha = nclip_plat::opacity::opacity_alpha(pct);
    let Ok(h) = win.window_handle() else {
        return Err("window_handle");
    };
    match h.as_raw() {
        #[cfg(windows)]
        RawWindowHandle::Win32(w) => nclip_plat::opacity::set_hwnd_alpha(w.hwnd.get(), alpha)
            .then_some(())
            .ok_or("SetLayeredWindowAttributes"),
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(a) => nclip_plat::opacity::set_ns_view_window_alpha(
            a.ns_view.as_ptr(),
            f64::from(alpha) / 255.0,
        )
        .then_some(())
        .ok_or("NSWindow"),
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xlib(x) => {
            nclip_plat::opacity::set_x11_opacity(x.window as u32, alpha).map_err(|_| "x11")
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        RawWindowHandle::Xcb(x) => {
            nclip_plat::opacity::set_x11_opacity(x.window.get(), alpha).map_err(|_| "x11")
        }
        _ => {
            let _ = alpha;
            Err("unsupported (Wayland)")
        }
    }
}

/// 설정 4키의 유효값(순수 · 클램프 적용).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Values {
    /// 포커스 있을 때(%) — 메인 · 보조 창 · 새 창.
    pub(crate) focused: i64,
    /// 포커스 없을 때(%) — 메인만.
    pub(crate) blur: i64,
    /// 자체 창(설정 · 팝업) 포커스 = 묶음 활성.
    pub(crate) keeps_active: bool,
    /// 흐리기 유예.
    pub(crate) grace: Duration,
}

impl Values {
    /// 설정에서 읽는다 — 값이 없거나 깨졌으면 레지스트리 기본(5 · 10 · on · 150).
    pub(crate) fn from_conf(conf: &crate::conf::Settings) -> Self {
        let n = |k: &str, d: i64| conf.state.get(k).trim().parse::<i64>().unwrap_or(d);
        Self {
            focused: n("ui.transparency", 5).clamp(0, OPACITY_MAX_PCT),
            blur: n("ui.transparency_blur", 10).clamp(0, OPACITY_MAX_PCT),
            keeps_active: conf.state.get("ui.dialog_keeps_active") != "off",
            grace: Duration::from_millis(n("ui.blur_grace_ms", 150).clamp(0, 2000) as u64),
        }
    }

    /// 변경 감지용 서명(셸이 박동마다 비교 · `hotkey_sig` 관례).
    pub(crate) fn sig(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.focused,
            self.blur,
            self.keeps_active,
            self.grace.as_millis()
        )
    }

    /// 메인창에 지금 적용할 값 — 묶음 활성이면 포커스 값, 아니면 blur 값.
    pub(crate) fn main_pct(&self, active: bool) -> i64 {
        if active {
            self.focused
        } else {
            self.blur
        }
    }
}

/// 박동 판정 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tick {
    /// 유예가 끝났고 여전히 묶음 밖 — 메인에 blur 값을 적용한다.
    Apply,
    /// 유예 중 — 이 시각에 다시 깨어난다.
    WakeAt(Instant),
    /// 할 일 없음.
    Idle,
}

/// 창 묶음(메인 + 보조 창) 포커스 상태 — 투명도는 "묶음 중 하나라도 포커스가 있는가"로 정한다(GAP-039).
/// 창 식별은 `u64`(winit `WindowId`의 `From<WindowId> for u64`) — 시험이 winit 없이 만든다.
#[derive(Debug, Default)]
pub(crate) struct FocusGroup {
    /// 메인창에 포커스가 있는가.
    main_active: bool,
    /// 포커스를 가진 보조 창(설정 · 팝업).
    aux_focus: Option<u64>,
    /// 흐리기 유예 만료 시각(유예 중일 때만).
    blur_at: Option<Instant>,
}

impl FocusGroup {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 메인창 포커스 전환 — 받았으면 보조 창 기억은 지운다(포커스는 한 창에만 · 닫힌 창의 남은 값 정리).
    pub(crate) fn set_main(&mut self, on: bool) {
        self.main_active = on;
        if on {
            self.aux_focus = None;
        }
    }

    /// 보조 창 포커스 전환 — 얻으면 그 창을 기억 · 잃으면(그 창일 때만) 잊는다.
    pub(crate) fn set_aux(&mut self, id: u64, on: bool) {
        if on {
            self.aux_focus = Some(id);
        } else if self.aux_focus == Some(id) {
            self.aux_focus = None;
        }
    }

    /// 사라진 창 정리 — 닫힌 창은 `Focused(false)`를 안 보낼 수 있다(셸이 창 존재로 대조해 부른다).
    pub(crate) fn prune(&mut self, main_exists: bool, aux_exists: impl Fn(u64) -> bool) {
        if !main_exists {
            self.main_active = false;
        }
        if self.aux_focus.is_some_and(|id| !aux_exists(id)) {
            self.aux_focus = None;
        }
    }

    /// 묶음 활성 — 메인 포커스 · 또는(`keeps_active`) 보조 창 포커스.
    pub(crate) fn active(&self, keeps_active: bool) -> bool {
        self.main_active || (self.aux_focus.is_some() && keeps_active)
    }

    /// 묶음 상태가 바뀐 뒤 — 활성이면 **바로 적용**(`true`) · 비활성이면 유예를 건다(이미 걸려 있으면 그대로).
    pub(crate) fn changed(&mut self, keeps_active: bool, grace: Duration, now: Instant) -> bool {
        if self.active(keeps_active) {
            self.blur_at = None;
            true
        } else {
            if self.blur_at.is_none() {
                self.blur_at = Some(now + grace);
            }
            false
        }
    }

    /// 박동 — 유예가 지났고 여전히 묶음 밖이면 [`Tick::Apply`] · 유예 중이면 깰 시각.
    pub(crate) fn tick(&mut self, keeps_active: bool, now: Instant) -> Tick {
        let Some(at) = self.blur_at else {
            return Tick::Idle;
        };
        if now < at {
            return Tick::WakeAt(at);
        }
        self.blur_at = None;
        if self.active(keeps_active) {
            Tick::Idle
        } else {
            Tick::Apply
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Duration = Duration::from_millis(150);

    /// dir3 `window_transparency_follows_setting_and_boost`의 묶음 판정부 — 메인 → 보조 창 전환은 유예로 넘기고,
    /// 앱 밖으로 나가면 유예 뒤 blur · 보조 창 포커스는 `keeps_active` 끔이면 묶음에 안 든다.
    #[test]
    fn focus_group_grace_and_dialog_keeps_active() {
        let t0 = Instant::now();
        let mut g = FocusGroup::new();
        g.set_main(true);
        assert!(g.changed(true, G, t0));
        assert!(g.active(true));
        // 메인 → 설정 창: 잃음(유예 시작) → 얻음(유예 취소 · 바로 포커스 값).
        g.set_main(false);
        assert!(!g.changed(true, G, t0));
        assert_eq!(
            g.tick(true, t0 + Duration::from_millis(10)),
            Tick::WakeAt(t0 + G)
        );
        g.set_aux(7, true);
        assert!(g.changed(true, G, t0 + Duration::from_millis(20)));
        assert_eq!(g.tick(true, t0 + Duration::from_secs(1)), Tick::Idle);
        // keeps_active 끔 = 보조 창 포커스는 묶음 밖 → 유예 뒤 blur.
        assert!(!g.active(false));
        assert!(!g.changed(false, G, t0));
        assert_eq!(g.tick(false, t0 + G), Tick::Apply);
        // 앱 밖으로: 보조 창도 잃음 → 유예 뒤 Apply · 유예 안에 돌아오면 Idle.
        g.set_aux(7, false);
        assert!(!g.changed(true, G, t0));
        g.set_main(true);
        assert_eq!(g.tick(true, t0 + G), Tick::Idle);
        g.set_main(false);
        assert!(!g.changed(true, G, t0));
        assert_eq!(g.tick(true, t0 + G), Tick::Apply);
        // 다른 보조 창의 잃음은 기억을 지우지 않는다 · 메인이 받으면 보조 기억은 지운다.
        g.set_aux(7, true);
        g.set_aux(9, false);
        assert!(g.active(true));
        g.set_main(true);
        g.set_main(false);
        assert!(!g.active(true));
        // 닫힌 보조 창 정리.
        g.set_aux(7, true);
        g.prune(true, |id| id != 7);
        assert!(!g.active(true));
    }

    /// 메인에 적용할 값 · 설정 서명.
    #[test]
    fn values_pick_main_pct_and_sig() {
        let v = Values {
            focused: 5,
            blur: 10,
            keeps_active: true,
            grace: G,
        };
        assert_eq!(v.main_pct(true), 5);
        assert_eq!(v.main_pct(false), 10);
        assert_eq!(v.sig(), "5|10|true|150");
    }
}
