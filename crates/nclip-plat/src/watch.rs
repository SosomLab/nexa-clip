//! 클립보드 감시 어댑터 — OS별 구현을 [`ClipboardWatch`] 뒤에 숨긴다.
//!
//! ## OS별 모델이 서로 다르다 ([docs/20 §3-1](../../../docs/20-implementation-spec.md))
//!
//! | OS | 방식 | 요점 |
//! |---|---|---|
//! | Windows | `AddClipboardFormatListener` → `WM_CLIPBOARDUPDATE` | **이벤트**. 숨은 메시지 창 필요 |
//! | macOS | `NSPasteboard.changeCount` **폴링** | 이벤트가 없다 — 적응형 주기 |
//! | X11 | `XFixesSelectSelectionInput` | 창이 닫히면 사라지므로 **즉시 읽는다** |
//! | Wayland | `zwlr/ext_data_control` | ★ **GNOME 미지원** → 정직하게 알린다 |
//!
//! ## 지금 상태
//!
//! 골격만 있다. 실물 구현은 T-14(3-OS 감시)에서 채운다 — 그때까지 [`capability`]는
//! [`UnsupportedReason::NotImplemented`]를 **정직하게** 돌려준다.
//! ⚠️ 조용히 빈 목록을 돌려주지 않는 것이 이 계층의 계약이다([docs/02 R-4]).

use nclip_core::{ClipSnapshot, ClipboardWatch, UnsupportedReason, WatchCapability, WatchError};

/// 이 타깃의 감시 어댑터.
///
/// 실물 구현 전이라 `start`는 실패하지만, **어떤 이유로 실패하는지**를 타입으로 말한다.
#[derive(Debug, Default)]
pub struct PlatformWatch {
    paused: bool,
    skip_next: bool,
}

impl PlatformWatch {
    /// 새 어댑터.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 일시 정지 상태(테스트·진단용).
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// 다음 1건 무시가 예약돼 있는가(테스트·진단용).
    #[must_use]
    pub fn is_skip_armed(&self) -> bool {
        self.skip_next
    }

    /// ★ 감시 루프가 스냅숏마다 부르는 게이트 — **저장할지 말지**를 여기서 한곳에 모은다.
    ///
    /// 순서가 중요하다:
    /// 1. **민감 표식이면 무조건 버린다**(FR-S-1 · fail-closed) — 일시정지보다 위다.
    /// 2. 일시 정지 중이면 버린다.
    /// 3. "다음 1건만 무시"가 예약돼 있으면 버리고 **예약을 해제**한다(FR-C-13).
    pub fn admit(&mut self, snap: &ClipSnapshot) -> bool {
        if snap.concealed {
            return false;
        }
        if self.paused {
            return false;
        }
        if self.skip_next {
            self.skip_next = false;
            return false;
        }
        true
    }
}

impl ClipboardWatch for PlatformWatch {
    fn capability(&self) -> WatchCapability {
        // 실물 구현이 붙기 전까지는 정직하게 "미구현"이다.
        WatchCapability::Unsupported {
            reason: UnsupportedReason::NotImplemented,
        }
    }

    fn start(&mut self, _on_change: Box<dyn Fn(ClipSnapshot) + Send>) -> Result<(), WatchError> {
        Err(WatchError::Unsupported(UnsupportedReason::NotImplemented))
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn skip_next(&mut self) {
        self.skip_next = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> ClipSnapshot {
        ClipSnapshot::default()
    }

    fn concealed() -> ClipSnapshot {
        ClipSnapshot {
            concealed: true,
            ..Default::default()
        }
    }

    #[test]
    fn admits_normal_snapshot() {
        let mut w = PlatformWatch::new();
        assert!(w.admit(&plain()));
    }

    /// ★ 민감 표식은 **일시정지 상태와 무관하게** 항상 막힌다(fail-closed).
    #[test]
    fn concealed_is_always_rejected() {
        let mut w = PlatformWatch::new();
        assert!(!w.admit(&concealed()));
        w.set_paused(true);
        assert!(!w.admit(&concealed()));
    }

    #[test]
    fn paused_rejects() {
        let mut w = PlatformWatch::new();
        w.set_paused(true);
        assert!(!w.admit(&plain()));
        w.set_paused(false);
        assert!(w.admit(&plain()));
    }

    /// ★ "다음 1건만 무시"는 **한 건만** 막고 스스로 풀린다 —
    /// 토글과 달리 다시 켜는 것을 잊을 수 없다.
    #[test]
    fn skip_next_consumes_exactly_one() {
        let mut w = PlatformWatch::new();
        w.skip_next();
        assert!(w.is_skip_armed());
        assert!(!w.admit(&plain()), "예약된 1건은 막힌다");
        assert!(!w.is_skip_armed(), "예약이 해제된다");
        assert!(w.admit(&plain()), "그 다음은 통과한다");
    }

    /// 미구현 타깃도 **이유를 말한다** — 조용한 빈 목록 금지.
    #[test]
    fn capability_is_honest() {
        let w = PlatformWatch::new();
        assert!(matches!(
            w.capability(),
            WatchCapability::Unsupported {
                reason: UnsupportedReason::NotImplemented
            }
        ));
    }
}
