//! GAP-042 — winit 0.30.13 `TaskbarCreated` 교착 우회(Windows 전용 · 앱 쪽 보완 · 11-04 nexa-dir3 이식).
//!
//! **결함**(상류 winit `platform_impl/windows/event_loop.rs:2455-2456` · PR #4563은 master(0.31)에만 병합):
//! 작업 표시줄이 다시 만들어질 때(RDP 재접속 · explorer 재시작) 셸이 등록 메시지 `TaskbarCreated`를 모든 최상위 창에
//! 브로드캐스트한다. winit은 이 메시지를 받으면 **창 상태 `Mutex` 가드를 쥔 채** `ITaskbarList::AddTab`을 부르고, 그 안의
//! `SendMessageW`(작업 표시줄 창)가 동기 대기한다. 대기 중 들어온 테마 변경 후처리(uxtheme `SetFrameTheme` → `SetWindowPos`)가
//! 같은 창에 `WM_WINDOWPOSCHANGING`을 동기 전달하고, winit이 같은 가드를 다시 잠그려다 영구 대기 → 창이 "응답 없음".
//!
//! **우회**: 창을 만든 직후 `SetWindowSubclass`(comctl32)로 서브클래스 프로시저를 단다. 서브클래스는 클래스 WndProc(winit)보다
//! **먼저** 메시지를 받으므로 `TaskbarCreated`만 `DefWindowProcW`로 돌려보내 winit의 처리(= AddTab)를 건너뛴다. 앱은
//! `skip_taskbar`를 쓰지 않으므로 AddTab은 필요 없고(탐색기가 다시 뜨면 최상위 창은 스스로 작업 표시줄에 오른다) 동작 차이가
//! 없다. 나머지 메시지는 `DefSubclassProc`로 그대로 흘려보낸다. `WM_NCDESTROY`에서 서브클래스를 뗀다(Raymond Chen 패턴).
//!
//! ⚠️ 트레이 창(`tray.rs`)은 이 메시지로 아이콘을 **재등록**해야 하므로 여기 가드를 달지 않는다 — 대상은 **winit 창만**.
//!
//! **적용 규칙**: winit 창을 만드는 모든 곳(`el.create_window(...)`)은 바로 뒤에 `taskbar_guard::guard_taskbar_created(&win)`을
//! 부른다(nexa-clip 본체) — 정적 가드 시험 `taskbar_guard_follows_every_window_creation`이 파일별 건수를 대조한다.
//! **제거 조건**: winit을 #4563이 든 판(0.31 이후)으로 올리면 이 모듈과 호출 지점 · 가드 시험을 지운다.
//!
//! 출처 = `../nexa-dir3/crates/nexa-dir/src/platform/wintaskbar.rs`(`windows` 크레이트 판) → 이 crate 규약(DR-8 외부 crate 0 ·
//! 순수 `extern "system"` · 핸들 `isize`)으로 옮겼다. 자족 안내 = nexa-dir3 `docs/27-winit-taskbar-deadlock.md`.

use std::sync::OnceLock;

use crate::win32::{DefWindowProcW, HWND, LPARAM, LRESULT, WPARAM};

/// 서브클래스 식별자("NCTB") — 같은 창에 두 번 달아도 comctl32가 같은 (proc, id) 쌍은 갱신만 한다.
const SUBCLASS_ID: usize = 0x4e43_5442;
/// 창 소멸 마지막 메시지.
const WM_NCDESTROY: u32 = 0x0082;

/// comctl32 `SUBCLASSPROC`.
type SubclassProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM, usize, usize) -> LRESULT;

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(hwnd: HWND, proc: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: HWND, proc: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
}

#[link(name = "user32")]
extern "system" {
    // tray.rs와 같은 서명(clashing_extern_declarations 회피).
    fn RegisterWindowMessageW(name: *const u16) -> u32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `RegisterWindowMessageW("TaskbarCreated")` — 세션 안에서 고정(셸과 같은 값 · 0xC000~0xFFFF).
fn taskbar_created_msg() -> u32 {
    static MSG: OnceLock<u32> = OnceLock::new();
    // SAFETY: 널 종료 UTF-16 · 실패 = 0(어떤 메시지와도 같지 않음).
    *MSG.get_or_init(|| unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) })
}

/// 창에 서브클래스를 단다 — 창을 만든 스레드(UI 스레드)에서 바로 부른다. 반환 = 성공 여부.
pub fn install(hwnd: isize) -> bool {
    // 등록 실패(0)면 걸러 낼 메시지가 없으니 달지 않는다(종전 그대로).
    if hwnd == 0 || taskbar_created_msg() == 0 {
        return false;
    }
    // SAFETY: 살아 있는 창의 HWND · 프로시저는 'static · 참조 자료 없음(0).
    unsafe { SetWindowSubclass(hwnd, guard_proc, SUBCLASS_ID, 0) != 0 }
}

unsafe extern "system" fn guard_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    if msg == taskbar_created_msg() {
        // winit의 처리(가드 쥔 채 AddTab 동기 대기)를 건너뛴다 — winit도 결국 DefWindowProc로 끝낸다.
        // SAFETY: 받은 인자를 그대로 넘기는 기본 처리.
        return unsafe { DefWindowProcW(hwnd, msg, w, l) };
    }
    if msg == WM_NCDESTROY {
        // SAFETY: 창 소멸 마지막 메시지 · 같은 (proc, id)로 뗀다 · 이후 DefSubclassProc 호출은 허용된 패턴.
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, guard_proc, id);
        }
    }
    // SAFETY: 서브클래스 사슬의 다음 프로시저(= winit WndProc)로 그대로 전달.
    unsafe { DefSubclassProc(hwnd, msg, w, l) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win32::{CreateWindowExW, GetModuleHandleW, RegisterClassW, HANDLE, WNDCLASSW};
    use std::sync::atomic::{AtomicU32, Ordering};

    const HWND_MESSAGE: HWND = -3;
    const WM_APP: u32 = 0x8000;

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageW(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn UnregisterClassW(class: *const u16, inst: HANDLE) -> i32;
    }

    /// 클래스 WndProc(= winit 자리)에 닿은 메시지 수 — TaskbarCreated · WM_APP+1 각각.
    static SEEN_TASKBAR: AtomicU32 = AtomicU32::new(0);
    static SEEN_APP: AtomicU32 = AtomicU32::new(0);

    unsafe extern "system" fn class_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == taskbar_created_msg() {
            SEEN_TASKBAR.fetch_add(1, Ordering::SeqCst);
        } else if msg == WM_APP + 1 {
            SEEN_APP.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: 기본 처리.
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    /// 메시지 전용 창(화면 없음)에 서브클래스를 달면 `TaskbarCreated`는 클래스 WndProc에 닿지 않고, 다른 메시지는 그대로 닿는다.
    /// 소멸(`WM_NCDESTROY`) 뒤에도 패닉 없이 끝난다.
    #[test]
    fn taskbar_created_is_swallowed_and_other_messages_pass_through() {
        let class = wide("nclip-taskbar-guard-test");
        // SAFETY: 이 시험 전용 클래스 · 메시지 전용 창 · 같은 스레드에서 만들고 지운다.
        unsafe {
            let inst = GetModuleHandleW(core::ptr::null());
            let wc = WNDCLASSW {
                style: 0,
                lpfn_wnd_proc: Some(class_proc),
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: inst,
                h_icon: 0,
                h_cursor: 0,
                hbr_background: 0,
                lpsz_menu_name: core::ptr::null(),
                lpsz_class_name: class.as_ptr(),
            };
            assert_ne!(RegisterClassW(&wc), 0, "RegisterClassW");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                0,
                inst,
                core::ptr::null(),
            );
            assert_ne!(hwnd, 0, "CreateWindowExW");
            let msg = taskbar_created_msg();
            assert!(msg >= 0xC000, "등록 메시지 범위");

            // 가드 전: 클래스 WndProc에 닿는다(= winit이 받던 상태).
            SendMessageW(hwnd, msg, 0, 0);
            assert_eq!(SEEN_TASKBAR.load(Ordering::SeqCst), 1);

            assert!(install(hwnd), "SetWindowSubclass");
            // 가드 뒤: TaskbarCreated는 걸러지고 다른 메시지는 통과.
            SendMessageW(hwnd, msg, 0, 0);
            SendMessageW(hwnd, WM_APP + 1, 0, 0);
            assert_eq!(SEEN_TASKBAR.load(Ordering::SeqCst), 1, "걸러져야 한다");
            assert_eq!(SEEN_APP.load(Ordering::SeqCst), 1, "통과해야 한다");
            // 두 번 달아도 갱신만(같은 proc · id) — 여전히 1건.
            assert!(install(hwnd));
            SendMessageW(hwnd, msg, 0, 0);
            assert_eq!(SEEN_TASKBAR.load(Ordering::SeqCst), 1);

            assert_ne!(DestroyWindow(hwnd), 0, "DestroyWindow");
            assert_ne!(
                UnregisterClassW(class.as_ptr(), inst),
                0,
                "UnregisterClassW"
            );
        }
    }

    /// 널 핸들은 달지 않는다.
    #[test]
    fn install_rejects_null_hwnd() {
        assert!(!install(0));
    }
}
