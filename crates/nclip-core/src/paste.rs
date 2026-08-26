//! 붙여넣기 포트 — ★ **이 제품의 존재 이유가 걸린 계층**(FR-P-2 · [K-1 리스크](../../../docs/02-roadmap.md)).
//!
//! ## 왜 어려운가
//!
//! 팝업이 뜨면 **포커스가 우리에게 온다**. 사용자가 항목을 고르면 우리는
//! **원래 있던 창으로 포커스를 돌려주고 붙여넣기 키를 넣어야** 한다. 이 왕복이
//! 3-OS에서 각각 다르고, **권한이 걸린다**([docs/20 §3-4](../../../docs/20-implementation-spec.md)).
//!
//! ```text
//! ① 단축키 수신
//! ② ★ 직전 포그라운드 창/앱을 기억      ← 팝업을 띄우기 "전"
//! ③ 팝업 표시(포커스 획득) · 사용자 선택
//! ④ 팝업 숨김
//! ⑤ ★ ②를 다시 활성화
//! ⑥ 붙여넣기 키 주입(Ctrl+V / ⌘V)
//! ```
//!
//! ## 실패는 조용하면 안 된다
//!
//! 권한이 없거나(macOS 손쉬운 사용) 프로토콜이 없으면(Wayland) **주입을 못 한다**.
//! 그때는 [`PasteCapability`]로 **미리** 알리고, 실제 동작은 *"클립보드에만 올림"* 으로
//! **정직하게 강등**한다(FR-P-1). 사용자가 왜 안 되는지 알아야 한다.

/// 붙여넣기 계층이 이 환경에서 무엇을 할 수 있는가.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PasteCapability {
    /// 포커스 복원 + 키 주입까지 가능.
    Full {
        /// 구현 이름(진단 표시용 — 예: `win32-sendinput` · `mac-cgevent`).
        backend: &'static str,
    },
    /// ★ **권한만 주면 된다** — 기능은 있으나 사용자가 허용해야 한다.
    /// macOS 손쉬운 사용(Accessibility)이 이 경우다.
    NeedsPermission {
        /// 구현 이름.
        backend: &'static str,
        /// 사용자에게 보여줄 안내(어느 설정을 켜야 하는지).
        hint: &'static str,
    },
    /// ★ **구조적으로 불가능** — 클립보드 적재까지만 한다(FR-P-1 강등).
    ClipboardOnly {
        /// 왜 불가능한지.
        reason: PasteUnsupported,
    },
}

impl PasteCapability {
    /// 키 주입까지 실제로 시도할 수 있는가(권한 대기 상태는 **아직 아니다**).
    #[must_use]
    pub fn can_inject(&self) -> bool {
        matches!(self, PasteCapability::Full { .. })
    }
}

/// 주입이 불가능한 이유.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PasteUnsupported {
    /// Wayland — 합성기 밖에서 키를 넣는 표준이 없다.
    WaylandNoInjection,
    /// 표시 서버에 연결할 수 없다.
    NoDisplayServer,
    /// 이 타깃은 아직 구현되지 않았다.
    NotImplemented,
}

/// 붙여넣기 실패.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PasteError {
    /// 이 환경에서는 불가능하다 — 호출자는 클립보드 적재로 강등한다.
    Unsupported(PasteUnsupported),
    /// 권한이 없다(사용자 안내 대상).
    PermissionDenied {
        /// 어느 설정을 켜야 하는지.
        hint: &'static str,
    },
    /// 기억해 둔 창이 이미 사라졌다(사용자가 닫았다).
    TargetGone,
    /// OS 호출 실패(진단 문자열).
    Os(String),
}

/// 붙여넣을 형식 — [docs/12 §5](../../../docs/12-clipboard-formats.md)의 4모드 중
/// **주입 단계에서 갈리는 두 가지**만 여기 있다(나머지는 클립보드에 무엇을 올릴지의 문제다).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PasteAs {
    /// 원본 그대로 — 클립보드에 올린 표현 전부를 그대로 둔다.
    #[default]
    Original,
    /// 평문으로 — 평문 표현 하나만 올린다(FR-P-3).
    Plain,
}

/// ★ **붙여넣기 포트**.
///
/// 구현은 `nclip-plat`에 있고, 본체가 조립 시점에 주입한다.
pub trait PasteInjector: core::fmt::Debug {
    /// 이 환경에서 무엇이 가능한지. **팝업을 띄우기 전에** 물어볼 수 있어야 한다.
    fn capability(&self) -> PasteCapability;

    /// ② 지금 포그라운드에 있는 창/앱을 기억한다. ★ **팝업을 띄우기 전에** 부른다.
    ///
    /// 기억할 대상이 없으면(포그라운드 없음) `false`.
    fn capture_focus(&mut self) -> bool;

    /// ⑤+⑥ 기억해 둔 대상을 다시 활성화하고 붙여넣기 키를 넣는다.
    ///
    /// # Errors
    /// 권한 부재·대상 소실·OS 실패 시 [`PasteError`]. 호출자는 **클립보드 적재로 강등**한다.
    fn restore_and_paste(&mut self, as_: PasteAs) -> Result<(), PasteError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_full_can_inject() {
        assert!(PasteCapability::Full { backend: "x" }.can_inject());
        // ★ 권한 대기는 "아직 못 한다" — 낙관적으로 시도하면 조용히 실패한다.
        assert!(!PasteCapability::NeedsPermission {
            backend: "x",
            hint: "y"
        }
        .can_inject());
        assert!(!PasteCapability::ClipboardOnly {
            reason: PasteUnsupported::WaylandNoInjection
        }
        .can_inject());
    }

    #[test]
    fn paste_as_defaults_to_original() {
        // 정보를 잃지 않는 쪽이 기본이다(D-31).
        assert_eq!(PasteAs::default(), PasteAs::Original);
    }
}
