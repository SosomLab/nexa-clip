//! Wayland 컴포지터 능력 조회 — **data-control 프로토콜이 있는가**(08-30 실기가 요구).
//!
//! 실측(08-30 · GNOME 50): data-control이 없는 컴포지터에서 `wl-paste`/`wl-copy`는 **매번 숨은
//! 창을 만들어 키보드 포커스를 받은 뒤** 셀렉션에 접근한다(Wayland 보안 모델 — 포커스 없는
//! 클라이언트는 클립보드를 못 본다). 500ms 폴링이면 **포커스가 끊임없이 뺏겼다 돌아오고**
//! Dock에 톱니바퀴 창이 깜빡인다 — 사용자가 타이핑을 못 할 지경이었다.
//!
//! 그래서 백엔드 선택은 환경 변수가 아니라 **레지스트리 사실**로 한다: `zwlr_data_control_manager_v1`
//! 또는 `ext_data_control_manager_v1`이 있으면 wl-clipboard(포커스 불요), 없으면 XWayland
//! (`xclip` — Mutter가 Wayland↔X11 셀렉션을 양방향 동기화한다 · 08-29 XWayland 7/7 실측).

/// `WAYLAND_DISPLAY`에 붙어 레지스트리 글로벌 이름을 모은다(연결 실패 = 빈 목록).
#[cfg(target_os = "linux")]
#[must_use]
pub fn globals() -> Vec<String> {
    imp::globals()
}

/// data-control(wlr 또는 ext) 유무 — 레지스트리 1회 왕복.
#[cfg(target_os = "linux")]
#[must_use]
pub fn has_data_control() -> bool {
    has_data_control_in(&globals())
}

/// Wayland가 없는 OS의 **test 빌드**용 상수 거짓 — `watch_linux`의 순수부를 3-OS에서
/// 컴파일하기 때문에 필요하다([`crate`] lib.rs의 `watch_linux` cfg 참조).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn has_data_control() -> bool {
    false
}

/// 순수 판정(테스트 대상).
#[must_use]
pub fn has_data_control_in(globals: &[String]) -> bool {
    globals
        .iter()
        .any(|g| g == "zwlr_data_control_manager_v1" || g == "ext_data_control_manager_v1")
}

#[cfg(target_os = "linux")]
mod imp {
    use wayland_client::protocol::wl_registry;
    use wayland_client::{Connection, Dispatch, QueueHandle};

    #[derive(Default)]
    struct St(Vec<String>);

    impl Dispatch<wl_registry::WlRegistry, ()> for St {
        fn event(
            st: &mut Self,
            _: &wl_registry::WlRegistry,
            ev: wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { interface, .. } = ev {
                st.0.push(interface);
            }
        }
    }

    pub(super) fn globals() -> Vec<String> {
        let Ok(conn) = Connection::connect_to_env() else {
            return Vec::new();
        };
        let mut q = conn.new_event_queue::<St>();
        let qh = q.handle();
        let _reg = conn.display().get_registry(&qh, ());
        let mut st = St::default();
        if q.roundtrip(&mut st).is_err() {
            return Vec::new();
        }
        st.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_control_detection() {
        let g = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(has_data_control_in(&g(&[
            "wl_compositor",
            "zwlr_data_control_manager_v1"
        ])));
        assert!(has_data_control_in(&g(&["ext_data_control_manager_v1"])));
        assert!(!has_data_control_in(&g(&["wl_compositor", "xdg_wm_base"])));
    }
}
