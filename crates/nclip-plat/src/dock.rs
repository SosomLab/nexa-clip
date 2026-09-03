//! Dock 노출 제어(T-12e mac · 09-03 사용자 "독에 표시할지 여부 선택").
//!
//! 이식 원본: `nexa-beep` `crates/nbeep-plat/src/dock.rs`(08-30 `ui.tray_hide_taskbar`).
//! Windows·Linux는 창 자체(숨김/파괴)로 작업표시줄에서 빠지지만 **macOS Dock 아이콘은
//! 앱 단위**라 창을 숨겨도 남는다 → `NSApplication` activation policy를 `Accessory`로
//! 내리면 Dock·⌘Tab에서 사라지고(메뉴바 트레이만 남음) `Regular`로 올리면 복귀한다.
//! 다른 OS는 no-op(호출부 cfg 분기 없음).

/// Dock 아이콘 표시 여부(mac) — **메인 스레드에서만**(AppKit 규약 · winit 이벤트 루프).
pub fn set_dock_visible(visible: bool) {
    imp::set_dock_visible(visible);
}

/// 앱을 앞으로(mac) — Accessory(독 숨김) 상태에서 트레이 "열기"가 창을 진짜 앞으로
/// 가져오려면 창 포커스 전에 **앱 활성화**가 필요하다. 다른 OS no-op.
pub fn activate_front() {
    imp::activate_front();
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;

    pub(super) fn set_dock_visible(visible: bool) {
        let Some(mtm) = MainThreadMarker::new() else {
            return; // 메인 스레드가 아니면 하지 않는다(fail-soft)
        };
        let app = NSApplication::sharedApplication(mtm);
        let policy = if visible {
            NSApplicationActivationPolicy::Regular
        } else {
            NSApplicationActivationPolicy::Accessory
        };
        app.setActivationPolicy(policy);
        if visible {
            // Accessory→Regular 직후엔 앱이 비활성일 수 있다 — 앞으로 가져온다.
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
    }

    pub(super) fn activate_front() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn set_dock_visible(_visible: bool) {}
    pub(super) fn activate_front() {}
}
