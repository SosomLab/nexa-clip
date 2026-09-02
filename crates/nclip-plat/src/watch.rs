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
//! | **macOS** | ✅ 구현([`crate::watch_mac`]) — `changeCount` 적응형 폴링 (T-14e) |
//! | **Linux** | ✅ 1단([`crate::watch_linux`]) — `wl-paste`/`xclip` 파이프 + 지문 폴링. 도구가 없으면 정직하게 알린다 |
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
    /// 2. ★ **내용이 없으면 버린다** — 빈 항목은 항목이 아니다(08-27 실기).
    /// 3. 일시 정지 중이면 버린다.
    /// 4. "다음 1건만 무시"가 예약돼 있으면 버리고 **예약을 해제**한다(FR-C-13).
    ///
    /// ⚠️ **2를 3보다 먼저** 둔다 — 일시정지 중에 빈 스냅숏이 와서 `skip_next`를
    /// 소모해 버리면, 정작 다음 진짜 복사가 저장된다.
    pub fn admit(&mut self, snap: &ClipSnapshot) -> bool {
        if snap.concealed {
            return false;
        }
        // ★ **내용이 없으면 항목이 아니다**(08-27 실기).
        //   Excel이 지연 렌더링으로 클립보드를 다시 채우는 순간에 읽히면 표현이 0개로 오고,
        //   rdpclip은 자기 표식(`Terminal Services Private Data`)만 올린다.
        //   ⚠️ 목록에 빈 줄이 쌓이면 사용자는 **제품이 고장 났다고 읽는다**.
        if !nclip_core::capture::has_content(&snap.reps) {
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
        #[cfg(target_os = "macos")]
        {
            crate::watch_mac::read_snapshot()
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            crate::watch_linux::read_snapshot()
        }
        #[cfg(not(any(windows, unix)))]
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
        #[cfg(target_os = "macos")]
        {
            WatchCapability::Supported {
                backend: "mac-changecount-poll",
            }
        }
        // Linux는 환경(표시 서버·도구)에 따라 갈린다 — 백엔드가 직접 판정한다.
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            crate::watch_linux::capability()
        }
        // 아직 구현하지 않은 타깃은 **정직하게** 미구현이라고 말한다.
        #[cfg(not(any(windows, unix)))]
        {
            WatchCapability::Unsupported {
                reason: nclip_core::UnsupportedReason::NotImplemented,
            }
        }
    }

    #[cfg_attr(not(any(windows, unix)), allow(unused_variables))]
    fn start(&mut self, on_change: Box<dyn Fn(ClipSnapshot) + Send>) -> Result<(), WatchError> {
        #[cfg(any(windows, unix))]
        {
            // ★ 게이트를 여기서 끼운다 — 감시 구현은 **무엇을 버릴지 모른다**.
            //   일시정지·민감 표식 판정은 OS 밖의 규칙이므로 콜백을 한 겹 감싼다.
            let gate = std::sync::Arc::clone(&self.gate);
            let sink: Box<dyn Fn(ClipSnapshot) + Send> = Box::new(move |snap| {
                // ⚠️ 락이 깨졌으면 **버린다**(fail-closed) — 판정을 못 하면 저장하지 않는다.
                let pass = gate.lock().is_ok_and(|mut g| g.admit(&snap));
                if pass {
                    on_change(snap);
                }
            });
            #[cfg(windows)]
            {
                crate::watch_win::start(sink)
            }
            #[cfg(target_os = "macos")]
            {
                crate::watch_mac::start(sink)
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                crate::watch_linux::start(sink)
            }
        }
        #[cfg(not(any(windows, unix)))]
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

    /// 내용이 있는 평범한 스냅숏.
    fn plain() -> ClipSnapshot {
        ClipSnapshot {
            reps: vec![nclip_core::RawRep {
                format: "CF_UNICODETEXT".into(),
                data: b"h\0i\0".to_vec(),
            }],
            ..Default::default()
        }
    }

    fn concealed() -> ClipSnapshot {
        ClipSnapshot {
            concealed: true,
            ..plain()
        }
    }

    #[test]
    fn admits_normal_snapshot() {
        let mut w = PlatformWatch::new();
        assert!(w.admit(&plain()));
    }

    /// ★ 내용이 없는 스냅숏은 **항목이 아니다**(08-27 실기 — Excel 0개 · rdpclip 표식뿐).
    #[test]
    fn empty_and_metadata_only_are_rejected() {
        let mut w = PlatformWatch::new();
        assert!(!w.admit(&ClipSnapshot::default()), "표현 0개");

        let meta_only = ClipSnapshot {
            reps: vec![nclip_core::RawRep {
                format: "Terminal Services Private Data".into(),
                data: vec![0; 4],
            }],
            ..Default::default()
        };
        assert!(!w.admit(&meta_only), "★ 곁다리만 있는 것도 항목이 아니다");
    }

    /// ⚠️ 빈 스냅숏이 **"다음 1건 무시"를 소모하면 안 된다**.
    ///
    /// 소모해 버리면 정작 다음 진짜 복사가 저장된다 — 무시를 건 이유가 사라진다.
    #[test]
    fn empty_snapshot_does_not_consume_skip_next() {
        let mut w = PlatformWatch::new();
        w.skip_next();
        assert!(!w.admit(&ClipSnapshot::default()));
        assert!(w.is_skip_armed(), "★ 빈 것 때문에 예약이 풀리면 안 된다");
        assert!(!w.admit(&plain()), "진짜 복사가 예약을 소모한다");
        assert!(!w.is_skip_armed());
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

    /// macOS는 폴링 백엔드를 **이름으로** 보고한다(T-14e).
    #[test]
    #[cfg(target_os = "macos")]
    fn capability_reports_backend_on_macos() {
        match PlatformWatch::new().capability() {
            WatchCapability::Supported { backend } => {
                assert_eq!(backend, "mac-changecount-poll");
            }
            WatchCapability::Unsupported { reason } => {
                panic!("macOS는 구현돼 있다: {reason:?}");
            }
        }
    }

    /// Linux는 환경에 따라 갈린다 — 어느 쪽이든 **판단 가능한 답**을 준다.
    ///
    /// (CI 러너는 헤드리스라 `NoDisplayServer`, 데스크톱은 도구 유무에 따라 갈린다 —
    /// 여기서는 "정직한 보고" 계약만 검증한다.)
    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn capability_answers_honestly_on_linux() {
        match PlatformWatch::new().capability() {
            WatchCapability::Supported { backend } => {
                assert!(
                    [
                        "wayland-wl-paste",
                        "x11-x11rb",
                        "xwayland-x11rb",
                        "x11-xclip",
                        "xwayland-xclip",
                    ]
                    .contains(&backend),
                    "{backend}"
                );
            }
            WatchCapability::Unsupported { reason } => {
                assert_ne!(
                    reason,
                    nclip_core::UnsupportedReason::NotImplemented,
                    "Linux 1단이 있으므로 '미구현'은 답이 아니다 — 환경 사유를 말해야 한다"
                );
            }
        }
    }
}
