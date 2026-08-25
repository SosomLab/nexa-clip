//! 포트 — **core가 선언하고 어댑터가 구현한다**(의존성 역전).
//!
//! `nclip-plat`이 OS별로 구현하고 본체(`nexa-clip`)가 조립 시점에 주입한다.
//! core는 Win32도 AppKit도 Wayland도 모른다.

use crate::item::Representation;

/// 한 번의 클립보드 변화에서 읽어낸 **표현 묶음**.
///
/// ★ 여기서 이미 [`Representation`] 목록이다 — **해석은 하지 않고 이름째** 담는다
/// ([docs/12 F-1](../../../docs/12-clipboard-formats.md)).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClipSnapshot {
    /// 보유 표현 전부.
    pub reps: Vec<Representation>,
    /// 출처 앱 표시 이름(알아낼 수 있으면).
    pub source_app: Option<String>,
    /// ★ **민감 표식이 붙어 있었다** — 비밀번호 관리자 등이 *"기록하지 마"* 를 보낸 경우.
    /// 이게 `true`면 **저장하지 않는다**(FR-S-1 · fail-closed).
    pub concealed: bool,
}

/// 감시 계층이 이 환경에서 **무엇을 할 수 있는가**.
///
/// ⚠️ 미지원을 조용한 빈 목록으로 숨기지 않기 위해 존재한다 —
/// Wayland/GNOME처럼 **구조적으로 불가능한 환경**을 사용자에게 정직하게 알린다
/// ([docs/02 R-4](../../../docs/02-roadmap.md)).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WatchCapability {
    /// 정상 감시 가능.
    Supported {
        /// 구현 이름(진단 표시용 — 예: `win32-listener` · `mac-pollling` · `x11-xfixes`).
        backend: &'static str,
    },
    /// 이 환경에서는 수집할 수 없다.
    Unsupported {
        /// 사용자에게 보여줄 사유(진단·안내용).
        reason: UnsupportedReason,
    },
}

/// 감시가 불가능한 이유.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnsupportedReason {
    /// Wayland 컴포지터가 data-control 프로토콜을 제공하지 않는다(GNOME 등).
    WaylandNoDataControl,
    /// 표시 서버에 연결할 수 없다(헤드리스 등).
    NoDisplayServer,
    /// 이 타깃은 아직 구현되지 않았다.
    NotImplemented,
}

/// 감시 시작 실패.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WatchError {
    /// 이 환경에서는 불가능하다(사용자 안내 대상).
    Unsupported(UnsupportedReason),
    /// OS 호출이 실패했다(진단 문자열).
    Os(String),
}

/// ★ **클립보드 감시 포트**(FR-C-1).
///
/// 구현은 OS마다 모델이 다르다 — Windows는 이벤트, macOS는 폴링, X11은 셀렉션 알림,
/// Wayland는 컴포지터 프로토콜([docs/20 §3-1](../../../docs/20-implementation-spec.md)).
/// **그 차이는 전부 이 트레이트 뒤에 있다.**
pub trait ClipboardWatch: core::fmt::Debug {
    /// 이 환경에서 무엇이 가능한지. **`start` 전에** 물어볼 수 있어야 한다(온보딩 점검).
    fn capability(&self) -> WatchCapability;

    /// 감시를 시작한다. 변화마다 `on_change`가 불린다.
    ///
    /// # Errors
    /// 환경이 지원하지 않거나 OS 호출이 실패하면 [`WatchError`].
    fn start(&mut self, on_change: Box<dyn Fn(ClipSnapshot) + Send>) -> Result<(), WatchError>;

    /// 일시 정지/재개(FR-C-11). 정지 중에는 `on_change`가 불리지 않는다.
    fn set_paused(&mut self, paused: bool);

    /// ★ **다음 1건만 무시**(FR-C-13 · Maccy 선례) —
    /// 토글과 달리 **다시 켜는 것을 잊을 수 없다**([docs/14 §2-2](../../../docs/14-settings-registry.md)).
    fn skip_next(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 미지원 환경도 **타입으로 표현된다** — 조용히 빈 목록이 되지 않는다.
    #[test]
    fn unsupported_carries_reason() {
        let cap = WatchCapability::Unsupported {
            reason: UnsupportedReason::WaylandNoDataControl,
        };
        match cap {
            WatchCapability::Unsupported { reason } => {
                assert_eq!(reason, UnsupportedReason::WaylandNoDataControl);
            }
            WatchCapability::Supported { .. } => panic!("지원으로 잘못 읽혔다"),
        }
    }

    /// 민감 표식이 붙은 스냅숏은 **기본이 저장 금지**임을 모델이 드러낸다.
    #[test]
    fn concealed_defaults_false_but_is_explicit() {
        let s = ClipSnapshot::default();
        assert!(!s.concealed);
        let c = ClipSnapshot {
            concealed: true,
            ..Default::default()
        };
        assert!(c.concealed);
    }
}
