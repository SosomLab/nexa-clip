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
//! | OS | 상태 |
//! |---|---|
//! | **Windows** | ✅ 구현([`crate::watch_win`]) — 메시지 전용 창 + `WM_CLIPBOARDUPDATE` |
//! | macOS · Linux | ☐ 미구현 — [`capability`]가 **정직하게** [`UnsupportedReason::NotImplemented`] |
//!
//! ⚠️ 조용히 빈 목록을 돌려주지 않는 것이 이 계층의 계약이다([docs/02 R-4]).
//!
//! ## ★ 게이트는 OS 밖에 있다
//!
//! [`PlatformWatch::admit`]은 OS 구현과 **무관한 순수 판정**이다. 그래서 세 OS 어디서든
//! 같은 규칙이 돌고, 실제 클립보드 없이 테스트된다. OS별 코드가 늘어도 *"민감 표식이
//! 일시정지보다 위"* 같은 규칙은 **한 군데서만** 바뀐다.

use nclip_core::{ClipSnapshot, ClipboardWatch, WatchCapability, WatchError};

/// 이 타깃의 감시 어댑터.
///
/// 실물 구현 전이라 `start`는 실패하지만, **어떤 이유로 실패하는지**를 타입으로 말한다.
#[derive(Debug, Default)]
pub struct PlatformWatch {
    /// ★ 감시 스레드와 **공유**하는 게이트 — 설정 창에서 일시정지를 눌러도 즉시 먹는다.
    gate: std::sync::Arc<std::sync::Mutex<Gate>>,
}

/// 저장 여부 판정 상태 — [`PlatformWatch::admit`]의 실체.
#[derive(Debug, Default)]
pub struct Gate {
    paused: bool,
    skip_next: bool,
}

impl Gate {
    /// ★ 스냅숏마다 부르는 게이트 — **저장할지 말지**를 여기 한곳에 모은다.
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

impl PlatformWatch {
    /// 새 어댑터.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 일시 정지 상태(테스트·진단용).
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.gate.lock().is_ok_and(|g| g.paused)
    }

    /// 다음 1건 무시가 예약돼 있는가(테스트·진단용).
    #[must_use]
    pub fn is_skip_armed(&self) -> bool {
        self.gate.lock().is_ok_and(|g| g.skip_next)
    }

    /// 게이트 판정(테스트·진단용 — 실제 경로는 감시 스레드가 부른다).
    pub fn admit(&mut self, snap: &ClipSnapshot) -> bool {
        self.gate.lock().is_ok_and(|mut g| g.admit(snap))
    }

    /// 지금 클립보드를 **한 번만** 읽는다(진단 · 첫 실행 시 기존 내용 담기).
    ///
    /// 구현이 없는 OS에서는 `None` — ★ 빈 스냅숏으로 위장하지 않는다.
    #[must_use]
    pub fn read_now(&self) -> Option<ClipSnapshot> {
        #[cfg(windows)]
        {
            crate::watch_win::read_snapshot()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

impl ClipboardWatch for PlatformWatch {
    fn capability(&self) -> WatchCapability {
        #[cfg(windows)]
        {
            WatchCapability::Supported {
                backend: "win32-listener",
            }
        }
        // 아직 구현하지 않은 OS는 **정직하게** 미구현이라고 말한다.
        #[cfg(not(windows))]
        {
            WatchCapability::Unsupported {
                reason: nclip_core::UnsupportedReason::NotImplemented,
            }
        }
    }

    #[cfg_attr(not(windows), allow(unused_variables))]
    fn start(&mut self, on_change: Box<dyn Fn(ClipSnapshot) + Send>) -> Result<(), WatchError> {
        #[cfg(windows)]
        {
            // ★ 게이트를 여기서 끼운다 — 감시 구현은 **무엇을 버릴지 모른다**.
            //   일시정지·민감 표식 판정은 OS 밖의 규칙이므로 콜백을 한 겹 감싼다.
            let gate = std::sync::Arc::clone(&self.gate);
            crate::watch_win::start(Box::new(move |snap| {
                // ⚠️ 락이 깨졌으면 **버린다**(fail-closed) — 판정을 못 하면 저장하지 않는다.
                let pass = gate.lock().is_ok_and(|mut g| g.admit(&snap));
                if pass {
                    on_change(snap);
                }
            }))
        }
        #[cfg(not(windows))]
        {
            Err(WatchError::Unsupported(
                nclip_core::UnsupportedReason::NotImplemented,
            ))
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if let Ok(mut g) = self.gate.lock() {
            g.paused = paused;
        }
    }

    fn skip_next(&mut self) {
        if let Ok(mut g) = self.gate.lock() {
            g.skip_next = true;
        }
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

    /// ★ 능력은 **정직해야 한다** — 구현이 있는 곳만 "지원"이라고 말한다.
    ///
    /// 조용한 빈 목록으로 얼버무리지 않는 것이 이 계층의 계약이다([docs/02 R-4]).
    /// ⚠️ 타깃별로 **기대가 다르므로** 테스트도 갈라 둔다 — 한 몸으로 쓰면
    /// *"어느 쪽이든 통과"* 가 되어 아무것도 못 지킨다.
    #[test]
    #[cfg(windows)]
    fn capability_reports_backend_on_windows() {
        match PlatformWatch::new().capability() {
            WatchCapability::Supported { backend } => {
                assert_eq!(backend, "win32-listener");
            }
            WatchCapability::Unsupported { reason } => {
                panic!("Windows는 구현돼 있다: {reason:?}");
            }
        }
    }

    /// 구현이 없는 타깃은 **이유를 말한다**.
    #[test]
    #[cfg(not(windows))]
    fn capability_reports_reason_without_impl() {
        match PlatformWatch::new().capability() {
            WatchCapability::Unsupported { reason } => {
                assert_eq!(reason, nclip_core::UnsupportedReason::NotImplemented);
            }
            WatchCapability::Supported { .. } => {
                panic!("구현이 없는데 지원이라고 말했다");
            }
        }
    }

    /// 구현이 없는 타깃에서 `read_now`는 **`None`** 이다 — 빈 스냅숏으로 위장하지 않는다.
    #[test]
    #[cfg(not(windows))]
    fn read_now_is_none_without_impl() {
        assert!(PlatformWatch::new().read_now().is_none());
    }
}
