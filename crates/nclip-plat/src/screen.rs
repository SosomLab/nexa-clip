//! 화면 기하 조회 — ★ **작업 영역**(작업표시줄·Dock 제외)이 필요할 때(09-01 사용자 실기
//! "팝업이 작업표시줄에 가린다").
//!
//! winit `MonitorHandle`은 전체 해상도만 준다 — 작업표시줄이 점유한 띠를 모른다.
//! Windows는 `MonitorFromPoint` + `GetMonitorInfoW`의 `rcWork`가 정답이고,
//! 다른 OS는 `None`(호출측이 모니터 전체로 폴백 — mac은 창 관리자가 Dock을 스스로
//! 피하고, Wayland는 위치 지정 자체가 컴포지터 몫이라 이 경로가 덜 절실하다).

/// (x, y)가 든 모니터의 **작업 영역**(작업표시줄 제외) — `(x, y, w, h)` 물리 px.
#[must_use]
pub fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    imp::work_area_at(x, y)
}

#[cfg(windows)]
mod imp {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RectW {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: RectW,
        work: RectW,
        flags: u32,
    }
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromPoint(pt: Point, flags: u32) -> isize;
        fn GetMonitorInfoW(mon: isize, info: *mut MonitorInfo) -> i32;
    }

    pub(super) fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
        // SAFETY: 구조체 크기를 채워 넘기는 표준 호출 — 실패는 0 반환으로 드러난다.
        unsafe {
            let mon = MonitorFromPoint(Point { x, y }, MONITOR_DEFAULTTONEAREST);
            if mon == 0 {
                return None;
            }
            let mut info = MonitorInfo {
                size: u32::try_from(core::mem::size_of::<MonitorInfo>()).ok()?,
                monitor: RectW::default(),
                work: RectW::default(),
                flags: 0,
            };
            if GetMonitorInfoW(mon, &mut info) == 0 {
                return None;
            }
            let w = info.work;
            Some((w.left, w.top, w.right - w.left, w.bottom - w.top))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub(super) fn work_area_at(_x: i32, _y: i32) -> Option<(i32, i32, i32, i32)> {
        None
    }
}
