//! 시스템 트레이 상주(T-12e · FR-U-2) — **플랫폼 어댑터**.
//!
//! 이식 원본: `nexa-beep` `crates/nbeep-plat/src/tray.rs`(1,006줄 · 3-OS).
//! ★ 이번 이식은 **공통 타입 + Windows 모듈**이다 — macOS는 objc2 계열 의존을
//! 들여오는 결정(DR-8 원장)이 필요하고 Linux(SNI/zbus)와 함께 실기 환경이 있을 때
//! 이식한다. 미이식 타깃은 `spawn`이 `None`을 돌려 **정직하게 없다**고 알린다.
//!
//! ## Windows 구현 노트 (beep journal/2026-08-15 분석 그대로)
//!
//! - `Shell_NotifyIconW` 콜백은 **창 프로시저**로만 온다 → **전용 스레드 + 보이지
//!   않는 일반 창**. 메시지 전용 창(HWND_MESSAGE)을 쓰지 않는 이유 =
//!   **TaskbarCreated 브로드캐스트를 못 받는다**(explorer 재시작 시 아이콘 재등록 불가 —
//!   감시 창(`watch_win`)과 다른 선택인 이유가 이것이다).
//! - 우클릭 메뉴 = 네이티브 `TrackPopupMenu` + `SetForegroundWindow` 선행
//!   (안 하면 바깥 클릭에 메뉴가 안 닫히는 고전 버그 — MSDN 명시).
//! - 아이콘 = RGBA→BGRA 32bpp + `CreateIconIndirect`. 갱신 후 이전 HICON 파괴(누수 방지).

/// 트레이에서 온 사용자 행동.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// 창 열기/복원(좌클릭 · 메뉴 "열기").
    Open,
    /// **대상이 있는 열기**(알림 클릭) — 값 = 호스트가 알림에 실어 보낸 불투명 토큰.
    OpenTarget(String),
    /// ★ 최근 항목 선택(T-18e) — 값 = [`TrayContent::recent`]의 인덱스(0 = 최신).
    Recent(usize),
    /// 앱 종료(메뉴 "종료").
    Quit,
}

/// 트레이 표시 내용 — 호스트가 만들어 넘긴다(이 모듈은 앱 도메인을 모른다).
#[derive(Clone, Debug, Default)]
pub struct TrayContent {
    /// 정사각 RGBA(straight alpha) — 권장 32×32.
    pub rgba: Vec<u8>,
    /// 한 변(px).
    pub side: u32,
    /// 툴팁(127자 초과는 절단).
    pub tooltip: String,
    /// 메뉴 헤더(비활성 — 표시 이름).
    pub name: String,
    /// "열기" 라벨(i18n — 호스트 주입).
    pub open_label: String,
    /// "종료" 라벨(i18n — 호스트 주입).
    pub quit_label: String,
    /// ★ 최근 항목 라벨(0 = 최신 · T-18e) — 클릭 시 [`TrayEvent::Recent`]로 돌아온다.
    ///   개수·글자수 절단은 호스트 몫(이 모듈은 목록 정책을 모른다).
    pub recent: Vec<String>,
}

/// 살아 있는 트레이 핸들 — 갱신 요청만 보낸다(실행은 트레이 스레드).
#[derive(Debug)]
pub struct TrayHandle {
    _priv: (),
}

#[cfg(windows)]
pub use win::spawn;

#[cfg(not(windows))]
/// 스텁(미이식 타깃) — 트레이 없음. 호스트는 `None`을 보고 정직하게 알린다.
pub fn spawn<F: Fn(TrayEvent) + Send + Sync + 'static>(
    _content: TrayContent,
    _on_event: F,
) -> Option<TrayHandle> {
    None
}

#[cfg(not(windows))]
impl TrayHandle {
    /// 표시 내용 갱신(스텁 — 도달 불가).
    pub fn update(&self, _content: TrayContent) {}
    /// 풍선 알림(스텁 — 도달 불가).
    pub fn notify(&self, _title: &str, _body: &str, _silent: bool, _target: &str) {}
}

#[cfg(windows)]
mod win {
    use super::{TrayContent, TrayEvent, TrayHandle};
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    // ★ 겹치는 Win32 선언은 [`crate::win32`] 한 곳에만 — 핸들 규약도 `isize`로 통일.
    use crate::win32::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetModuleHandleW,
        PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, TranslateMessage,
        HANDLE, HWND, LPARAM, LRESULT, MSG, WNDCLASSW, WPARAM,
    };

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    /// `NOTIFYICONDATAW`(V4 크기 — 레거시 콜백 시맨틱 사용: lParam = 마우스 메시지).
    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: HWND,
        uid: u32,
        flags: u32,
        callback_message: u32,
        icon: HANDLE,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid: [u8; 16],
        balloon_icon: HANDLE,
    }

    #[repr(C)]
    struct IconInfo {
        f_icon: i32,
        x_hotspot: u32,
        y_hotspot: u32,
        bm_mask: HANDLE,
        bm_color: HANDLE,
    }

    #[link(name = "user32")]
    extern "system" {
        fn CreatePopupMenu() -> HANDLE;
        fn AppendMenuW(menu: HANDLE, flags: u32, id: usize, label: *const u16) -> i32;
        fn TrackPopupMenu(
            menu: HANDLE,
            flags: u32,
            x: i32,
            y: i32,
            reserved: i32,
            hwnd: HWND,
            rect: *const core::ffi::c_void,
        ) -> i32;
        fn DestroyMenu(menu: HANDLE) -> i32;
        fn GetCursorPos(pt: *mut Point) -> i32;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn CreateIconIndirect(info: *const IconInfo) -> HANDLE;
        fn DestroyIcon(icon: HANDLE) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(message: u32, data: *mut NotifyIconDataW) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateBitmap(
            w: i32,
            h: i32,
            planes: u32,
            bits_per_px: u32,
            bits: *const core::ffi::c_void,
        ) -> HANDLE;
        fn DeleteObject(obj: HANDLE) -> i32;
    }

    const WM_APP_CALLBACK: u32 = 0x8000 + 1; // WM_APP+1 — Shell_NotifyIcon 콜백
    const WM_APP_UPDATE: u32 = 0x8000 + 2; // 호스트 갱신 요청(상태는 STATE에)
    const WM_APP_BALLOON: u32 = 0x8000 + 3; // 풍선 알림 요청(내용은 BALLOON에)
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_DESTROY: u32 = 0x0002;
    const NIM_ADD: u32 = 0;
    const NIM_MODIFY: u32 = 1;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 0x01;
    const NIF_ICON: u32 = 0x02;
    const NIF_TIP: u32 = 0x04;
    const NIF_INFO: u32 = 0x10; // 풍선(info/info_title/info_flags 유효)
    const NIIF_INFO: u32 = 0x01;
    const NIIF_NOSOUND: u32 = 0x10;
    /// 풍선 클릭(레거시 콜백 lParam) — 알림 클릭 = 앱 열기.
    const NIN_BALLOONUSERCLICK: u32 = 0x0405;
    const MF_STRING: u32 = 0x0000;
    const MF_GRAYED: u32 = 0x0001;
    const MF_SEPARATOR: u32 = 0x0800;
    const TPM_RETURNCMD: u32 = 0x0100;
    const TPM_RIGHTBUTTON: u32 = 0x0002;
    const CMD_OPEN: usize = 1;
    const CMD_QUIT: usize = 2;
    /// 최근 항목 명령 id 시작(개수는 호스트가 준 목록 길이).
    const CMD_RECENT_BASE: usize = 100;

    /// 공유 상태 — wndproc(정적 fn)과 핸들이 같은 내용을 본다. 트레이는 프로세스당
    /// 1개(앱 창 하나의 부속)라 전역이 곧 인스턴스다.
    static STATE: OnceLock<Mutex<TrayContent>> = OnceLock::new();
    static ON_EVENT: OnceLock<Box<dyn Fn(TrayEvent) + Send + Sync>> = OnceLock::new();
    static HWND: AtomicIsize = AtomicIsize::new(0);
    static PREV_ICON: AtomicIsize = AtomicIsize::new(0);
    static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
    /// 대기 중 풍선 — (제목, 본문, 무음, 대상 토큰). 마지막 것만 유효.
    static BALLOON: Mutex<Option<(String, String, bool, String)>> = Mutex::new(None);
    /// 마지막 표시 풍선의 대상(클릭 복귀용 — 풍선은 아이콘당 1개라 마지막이 곧 화면).
    static LAST_TARGET: Mutex<String> = Mutex::new(String::new());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn emit(ev: TrayEvent) {
        if let Some(cb) = ON_EVENT.get() {
            cb(ev);
        }
    }

    /// RGBA(straight) → HICON. 실패 시 0(아이콘 없이도 등록은 진행 — fail-soft).
    fn hicon_from_rgba(rgba: &[u8], side: u32) -> HANDLE {
        let px = (side * side) as usize;
        if side == 0 || rgba.len() < px * 4 {
            return 0;
        }
        // BGRA로 채널 교환(GDI 비트맵 순서).
        let mut bgra = Vec::with_capacity(px * 4);
        for p in rgba[..px * 4].chunks_exact(4) {
            bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        // SAFETY: 32bpp 색 비트맵 + 형식상 마스크로 아이콘을 만든다. 생성물(비트맵)은
        // CreateIconIndirect가 복사하므로 즉시 파괴한다.
        unsafe {
            let side_i = i32::try_from(side).unwrap_or(32);
            let color = CreateBitmap(side_i, side_i, 1, 32, bgra.as_ptr().cast());
            if color == 0 {
                return 0;
            }
            let mask = CreateBitmap(side_i, side_i, 1, 1, core::ptr::null());
            let info = IconInfo {
                f_icon: 1,
                x_hotspot: 0,
                y_hotspot: 0,
                bm_mask: mask,
                bm_color: color,
            };
            let icon = CreateIconIndirect(&info);
            DeleteObject(color);
            if mask != 0 {
                DeleteObject(mask);
            }
            icon
        }
    }

    /// 현재 STATE를 트레이에 반영(NIM_ADD/MODIFY 공용). 이전 아이콘은 파괴.
    fn apply_state(hwnd: HWND, op: u32) {
        let Some(state) = STATE.get() else { return };
        let c = match state.lock() {
            Ok(g) => g.clone(),
            Err(_) => return,
        };
        let icon = hicon_from_rgba(&c.rgba, c.side);
        let mut nid = NotifyIconDataW {
            cb_size: u32::try_from(core::mem::size_of::<NotifyIconDataW>()).unwrap_or(0),
            hwnd,
            uid: 1,
            flags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            callback_message: WM_APP_CALLBACK,
            icon,
            tip: [0u16; 128],
            state: 0,
            state_mask: 0,
            info: [0u16; 256],
            version: 0,
            info_title: [0u16; 64],
            info_flags: 0,
            guid: [0u8; 16],
            balloon_icon: 0,
        };
        for (i, u) in c.tooltip.encode_utf16().take(127).enumerate() {
            nid.tip[i] = u;
        }
        // SAFETY: nid는 위에서 완전 초기화된 로컬 — 호출 동안만 참조된다.
        unsafe {
            Shell_NotifyIconW(op, &mut nid);
        }
        // 이전 HICON 파괴(누수 방지) — 새것을 슬롯에 보관.
        let prev = PREV_ICON.swap(icon, Ordering::AcqRel);
        if prev != 0 {
            // SAFETY: 우리가 만든 HICON이며 트레이는 복사본을 쓴다(NIM 반영 후 파괴 안전).
            unsafe {
                DestroyIcon(prev);
            }
        }
    }

    /// 우클릭 메뉴 — 이름 헤더(비활성) · ★ **최근 항목**(T-18e) · 열기 · 종료.
    fn show_menu(hwnd: HWND) {
        let Some(state) = STATE.get() else { return };
        let (name, open_label, quit_label, recent) = match state.lock() {
            Ok(g) => (
                g.name.clone(),
                g.open_label.clone(),
                g.quit_label.clone(),
                g.recent.clone(),
            ),
            Err(_) => return,
        };
        let name_w = wide(&name);
        let open_w = wide(&open_label);
        let quit_w = wide(&quit_label);
        let recent_w: Vec<Vec<u16>> = recent.iter().map(|s| wide(s)).collect();
        // SAFETY: 메뉴는 이 함수 안에서 만들고 파괴한다. SetForegroundWindow 선행은
        // TrackPopupMenu 관례(안 하면 바깥 클릭에 메뉴가 닫히지 않는다 — MSDN).
        unsafe {
            SetForegroundWindow(hwnd);
            let menu = CreatePopupMenu();
            if menu == 0 {
                return;
            }
            if !name.is_empty() {
                AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, name_w.as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
            }
            // ★ 최근 항목 — 최신이 위(0). 클릭 = 그 항목을 클립보드로.
            if !recent_w.is_empty() {
                for (i, w) in recent_w.iter().enumerate() {
                    AppendMenuW(menu, MF_STRING, CMD_RECENT_BASE + i, w.as_ptr());
                }
                AppendMenuW(menu, MF_SEPARATOR, 0, core::ptr::null());
            }
            AppendMenuW(menu, MF_STRING, CMD_OPEN, open_w.as_ptr());
            AppendMenuW(menu, MF_STRING, CMD_QUIT, quit_w.as_ptr());
            let mut pt = Point { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                0,
                hwnd,
                core::ptr::null(),
            );
            DestroyMenu(menu);
            match cmd as usize {
                CMD_OPEN => emit(TrayEvent::Open),
                CMD_QUIT => emit(TrayEvent::Quit),
                c if c >= CMD_RECENT_BASE && c < CMD_RECENT_BASE + recent_w.len() => {
                    emit(TrayEvent::Recent(c - CMD_RECENT_BASE));
                }
                _ => {}
            }
        }
    }

    /// 대기 중 풍선을 표시 — NIF_INFO만 갱신(아이콘·툴팁 불변). 제목 63자·본문 255자
    /// 절단(u16 셀 마지막은 NUL). 무음 = NIIF_NOSOUND.
    fn show_balloon(hwnd: HWND) {
        let Some((title, body, silent, target)) = BALLOON.lock().ok().and_then(|mut g| g.take())
        else {
            return;
        };
        if let Ok(mut t) = LAST_TARGET.lock() {
            *t = target;
        }
        let mut nid = NotifyIconDataW {
            cb_size: u32::try_from(core::mem::size_of::<NotifyIconDataW>()).unwrap_or(0),
            hwnd,
            uid: 1,
            flags: NIF_INFO,
            callback_message: 0,
            icon: 0,
            tip: [0u16; 128],
            state: 0,
            state_mask: 0,
            info: [0u16; 256],
            version: 0,
            info_title: [0u16; 64],
            info_flags: NIIF_INFO | if silent { NIIF_NOSOUND } else { 0 },
            guid: [0u8; 16],
            balloon_icon: 0,
        };
        for (i, u) in title.encode_utf16().take(63).enumerate() {
            nid.info_title[i] = u;
        }
        for (i, u) in body.encode_utf16().take(255).enumerate() {
            nid.info[i] = u;
        }
        // SAFETY: 살아 있는 트레이 아이콘(uid 1)의 풍선 필드만 수정.
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &mut nid);
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_APP_CALLBACK => {
                // 레거시 시맨틱 — lParam = 마우스 메시지.
                #[allow(clippy::cast_sign_loss)]
                match l as u32 {
                    WM_LBUTTONUP => emit(TrayEvent::Open),
                    WM_RBUTTONUP => show_menu(hwnd),
                    // 풍선 알림 클릭 = 열기(대상 토큰이 있으면 그쪽으로).
                    NIN_BALLOONUSERCLICK => {
                        let t = LAST_TARGET.lock().map(|g| g.clone()).unwrap_or_default();
                        if t.is_empty() {
                            emit(TrayEvent::Open);
                        } else {
                            emit(TrayEvent::OpenTarget(t));
                        }
                    }
                    _ => {}
                }
                0
            }
            WM_APP_UPDATE => {
                apply_state(hwnd, NIM_MODIFY);
                0
            }
            WM_APP_BALLOON => {
                show_balloon(hwnd);
                0
            }
            WM_DESTROY => {
                let mut nid = NotifyIconDataW {
                    cb_size: u32::try_from(core::mem::size_of::<NotifyIconDataW>()).unwrap_or(0),
                    hwnd,
                    uid: 1,
                    flags: 0,
                    callback_message: 0,
                    icon: 0,
                    tip: [0u16; 128],
                    state: 0,
                    state_mask: 0,
                    info: [0u16; 256],
                    version: 0,
                    info_title: [0u16; 64],
                    info_flags: 0,
                    guid: [0u8; 16],
                    balloon_icon: 0,
                };
                // SAFETY: 창 파괴 시 아이콘 제거 — nid는 로컬 완전 초기화.
                unsafe {
                    Shell_NotifyIconW(NIM_DELETE, &mut nid);
                    PostQuitMessage(0);
                }
                0
            }
            // explorer 재시작(TaskbarCreated 브로드캐스트) — 아이콘 재등록.
            m if TASKBAR_CREATED.get() == Some(&m) => {
                apply_state(hwnd, NIM_ADD);
                0
            }
            // SAFETY: 나머지는 기본 처리 위임.
            _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
        }
    }

    /// 트레이 스레드 기동 — 성공 시 핸들(갱신 통로). 프로세스당 1회(재호출 = None).
    pub fn spawn<F: Fn(TrayEvent) + Send + Sync + 'static>(
        content: TrayContent,
        on_event: F,
    ) -> Option<TrayHandle> {
        if STATE.set(Mutex::new(content)).is_err() {
            return None; // 이미 떠 있다
        }
        let _ = ON_EVENT.set(Box::new(on_event));
        std::thread::Builder::new()
            .name("nclip-tray".into())
            .spawn(|| {
                let class_name = wide("NexaClipTray");
                // SAFETY: 클래스 등록 → 보이지 않는 일반 창 생성 → 메시지 루프.
                // 창을 만들지 못하면 스레드만 조용히 끝난다(fail-soft — 앱은 트레이
                // 없이 동작).
                unsafe {
                    let instance = GetModuleHandleW(core::ptr::null());
                    let wc = WNDCLASSW {
                        style: 0,
                        lpfn_wnd_proc: Some(wndproc),
                        cb_cls_extra: 0,
                        cb_wnd_extra: 0,
                        h_instance: instance,
                        h_icon: 0,
                        h_cursor: 0,
                        hbr_background: 0,
                        lpsz_menu_name: core::ptr::null(),
                        lpsz_class_name: class_name.as_ptr(),
                    };
                    if RegisterClassW(&wc) == 0 {
                        return;
                    }
                    let _ = TASKBAR_CREATED
                        .set(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()));
                    let hwnd = CreateWindowExW(
                        0,
                        class_name.as_ptr(),
                        class_name.as_ptr(),
                        0, // WS_OVERLAPPED · 표시 안 함
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        instance,
                        core::ptr::null(),
                    );
                    if hwnd == 0 {
                        return;
                    }
                    HWND.store(hwnd, Ordering::Release);
                    apply_state(hwnd, NIM_ADD);
                    let mut msg = MSG {
                        hwnd: 0,
                        message: 0,
                        w_param: 0,
                        l_param: 0,
                        time: 0,
                        pt_x: 0,
                        pt_y: 0,
                    };
                    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .ok()?;
        Some(TrayHandle { _priv: () })
    }

    impl TrayHandle {
        /// 표시 내용 갱신(툴팁·아이콘 변경) — 트레이 스레드가 반영한다.
        pub fn update(&self, content: TrayContent) {
            if let Some(state) = STATE.get() {
                if let Ok(mut g) = state.lock() {
                    *g = content;
                }
            }
            let hwnd = HWND.load(Ordering::Acquire);
            if hwnd != 0 {
                // SAFETY: 살아 있는 트레이 창으로 갱신 통지만 보낸다.
                unsafe {
                    PostMessageW(hwnd, WM_APP_UPDATE, 0, 0);
                }
            }
        }

        /// 풍선 알림 — 트레이 스레드가 표시.
        /// `target` = 클릭 시 되돌아올 토큰(빈 문자열 = 대상 없음).
        pub fn notify(&self, title: &str, body: &str, silent: bool, target: &str) {
            if let Ok(mut g) = BALLOON.lock() {
                *g = Some((
                    title.to_string(),
                    body.to_string(),
                    silent,
                    target.to_string(),
                ));
            }
            let hwnd = HWND.load(Ordering::Acquire);
            if hwnd != 0 {
                // SAFETY: 살아 있는 트레이 창으로 표시 요청만 보낸다.
                unsafe {
                    PostMessageW(hwnd, WM_APP_BALLOON, 0, 0);
                }
            }
        }
    }
}
