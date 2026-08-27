//! Windows 클립보드 감시 — `AddClipboardFormatListener` (T-14b).
//!
//! 외부 crate 없이 Win32를 `#[link]` + `extern`으로 직접 부른다(DR-8).
//!
//! ## 왜 전용 스레드인가
//!
//! `AddClipboardFormatListener`는 **창**에 붙고, 알림은 **그 창의 메시지 큐**로 온다.
//! UI 이벤트 루프에 얹으면 감시가 창 생명주기에 묶인다 — 설정 창을 닫으면 감시가 멎는다.
//! ★ 그래서 **메시지 전용 창**(`HWND_MESSAGE`)을 **자기 스레드**에 띄운다.
//! 그 창은 화면에 없고, 입력도 안 받고, 오직 `WM_CLIPBOARDUPDATE`만 받는다.
//!
//! ## ⚠️ 클립보드는 **한 번에 한 프로세스**만 연다
//!
//! `OpenClipboard`는 다른 앱이 잡고 있으면 실패한다. 복사 직후에는 **복사한 앱이 아직
//! 잡고 있는 일이 흔하다**. 실패를 그대로 흘리면 *"가끔 복사가 안 잡히는"* 유령 버그가 된다
//! → [`open_with_retry`]가 짧게 물러섰다 다시 시도한다.
//!
//! ## ⚠️ 핸들 포맷은 `GlobalLock`으로 읽으면 안 된다
//!
//! 대부분의 포맷은 `HGLOBAL`(메모리 블록)이지만 `CF_BITMAP`은 `HBITMAP`,
//! `CF_ENHMETAFILE`은 `HENHMETAFILE`, `CF_PALETTE`는 `HPALETTE`다.
//! 이것들에 `GlobalSize`를 부르면 **엉뚱한 값이 나오거나 죽는다**.
//! → [`is_handle_format`]으로 걸러 **이름만** 담는다(날바이트는 빈 벡터).
//! 분류는 이름만 보므로([`nclip_core::capture`]) 종류 판정에는 지장이 없다.

// ★ Win32 타입 이름은 **원문 그대로** 둔다(`HWND`·`LPARAM`…).
// 러스트 관례로 고쳐 쓰면(`Hwnd`) MSDN 문서와 대조가 어려워지고, 시그니처를
// 눈으로 검증하기가 나빠진다. FFI 선언부에 한해 린트를 끈다.
#![allow(clippy::upper_case_acronyms)]

use nclip_core::{ClipSnapshot, RawRep, WatchError};
use std::sync::mpsc;

// ★ 겹치는 Win32 선언은 [`crate::win32`] **한 곳에만** 둔다 — 다른 시그니처로
// 두 번 선언하면 `clashing_extern_declarations`(우리 설정에서 오류)다.
use crate::win32::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetModuleHandleW,
    RegisterClassW, HANDLE, HWND, LPARAM, LRESULT, MSG, WNDCLASSW, WPARAM,
};

/// 변화가 있을 때 부를 것 — 스레드를 건너가므로 `Send`.
pub type Sink = Box<dyn Fn(ClipSnapshot) + Send>;

// ───────────────────────────── Win32 선언(이 모듈 전용)

const WM_CLIPBOARDUPDATE: u32 = 0x031D;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
/// 메시지 전용 창의 부모 — 화면에 뜨지 않는다.
const HWND_MESSAGE: HWND = -3;

/// 읽기 실패 재시도 타이머(§감시 루프). id는 이 창에서만 유일하면 된다.
const RETRY_TIMER_ID: usize = 1;
/// 재시도 간격 — 복사한 앱이 클립보드를 놓기를 기다리는 시간.
const RETRY_MS: u32 = 200;
/// ★ 재시도 횟수 상한 — 합쳐 약 3초. 그 뒤에도 못 열면 그 변화는 포기한다.
const RETRY_MAX: u32 = 15;

/// ★ **유실 안전망 하트비트** — `WM_CLIPBOARDUPDATE`가 **안 오는 일이 실제로 있다**
/// (08-27 실기: 탐색기 복사가 간헐적으로 유실 — 같은 절차가 잡히기도 안 잡히기도 했다).
///
/// 2초마다 `GetClipboardSequenceNumber`(클립보드를 **열지 않는** 시스템 호출 하나)만 비교하고,
/// 마지막으로 처리한 번호와 다를 때만 실제로 읽는다 — 유휴 비용은 2초당 호출 1회다(DR-9).
const POLL_TIMER_ID: usize = 2;
/// 하트비트 간격.
const POLL_MS: u32 = 2000;

/// 창 없음 / 실패를 뜻하는 핸들 값.
const NULL_HANDLE: HANDLE = 0;

#[link(name = "user32")]
extern "system" {
    fn AddClipboardFormatListener(hwnd: HWND) -> i32;
    fn OpenClipboard(hwnd: HWND) -> i32;
    fn CloseClipboard() -> i32;
    fn EnumClipboardFormats(format: u32) -> u32;
    fn GetClipboardData(format: u32) -> HANDLE;
    fn GetClipboardFormatNameW(format: u32, buf: *mut u16, cch: i32) -> i32;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
    fn GetClipboardSequenceNumber() -> u32;
    fn GetClipboardOwner() -> HWND;
    fn GetWindowThreadProcessId(hwnd: HWND, pid: *mut u32) -> u32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn SetTimer(hwnd: HWND, id: usize, elapse_ms: u32, callback: isize) -> usize;
    fn KillTimer(hwnd: HWND, id: usize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalLock(h: HANDLE) -> *mut core::ffi::c_void;
    fn GlobalUnlock(h: HANDLE) -> i32;
    fn GlobalSize(h: HANDLE) -> usize;
    fn Sleep(ms: u32);
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> HANDLE;
    fn CloseHandle(h: HANDLE) -> i32;
    fn QueryFullProcessImageNameW(h: HANDLE, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
}

/// `PROCESS_QUERY_LIMITED_INFORMATION` — 이름만 알면 되므로 **가장 약한 권한**.
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

// ───────────────────────────── 표준 포맷 이름표

/// 번호로 오는 표준 포맷의 **이름**([docs/12 §2](../../../docs/12-clipboard-formats.md)).
///
/// `GetClipboardFormatNameW`는 **등록 포맷에만** 이름을 준다 — 표준 번호는 빈 문자열이다.
/// 이름이 없으면 분류가 전부 "벤더"가 되므로 여기서 채운다.
fn standard_name(fmt: u32) -> Option<&'static str> {
    Some(match fmt {
        1 => "CF_TEXT",
        2 => "CF_BITMAP",
        3 => "CF_METAFILEPICT",
        4 => "CF_SYLK",
        5 => "CF_DIF",
        6 => "CF_TIFF",
        7 => "CF_OEMTEXT",
        8 => "CF_DIB",
        9 => "CF_PALETTE",
        10 => "CF_PENDATA",
        11 => "CF_RIFF",
        12 => "CF_WAVE",
        13 => "CF_UNICODETEXT",
        14 => "CF_ENHMETAFILE",
        15 => "CF_HDROP",
        16 => "CF_LOCALE",
        17 => "CF_DIBV5",
        _ => return None,
    })
}

/// ⚠️ `HGLOBAL`이 **아닌** 포맷 — `GlobalLock`/`GlobalSize`를 부르면 안 된다.
fn is_handle_format(fmt: u32) -> bool {
    matches!(fmt, 2 | 3 | 9 | 14) // CF_BITMAP · CF_METAFILEPICT · CF_PALETTE · CF_ENHMETAFILE
}

// ───────────────────────────── 문자열 도우미

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

// ───────────────────────────── 민감 표식

/// ★ 이 표식이 하나라도 있으면 **저장하지 않는다**(FR-S-1 · fail-closed).
///
/// | 표식 | 누가 쓰나 |
/// |---|---|
/// | `ExcludeClipboardContentFromMonitorProcessing` | 비밀번호 관리자 — *"감시 도구는 손대지 마"* |
/// | `CanIncludeInClipboardHistory` = 0 | Windows 클립보드 기록에서 제외 |
/// | `CanUploadToCloudClipboard` = 0 | 클라우드 동기화에서 제외 |
///
/// ⚠️ 앞의 것은 **존재만으로** 금지, 뒤의 둘은 **값이 0일 때** 금지다.
/// 값을 안 읽고 존재만 보면 *"기록해도 된다(=1)"* 를 금지로 잘못 읽는다.
fn read_concealed() -> bool {
    const EXCLUDE: &str = "ExcludeClipboardContentFromMonitorProcessing";
    const HISTORY: &str = "CanIncludeInClipboardHistory";
    const CLOUD: &str = "CanUploadToCloudClipboard";

    // SAFETY: 클립보드는 이미 열려 있다(호출자 계약). 포맷 등록은 부작용이 없다.
    unsafe {
        let ex = RegisterClipboardFormatW(wide(EXCLUDE).as_ptr());
        if ex != 0 && IsClipboardFormatAvailable(ex) != 0 {
            return true;
        }
        for name in [HISTORY, CLOUD] {
            let f = RegisterClipboardFormatW(wide(name).as_ptr());
            if f == 0 || IsClipboardFormatAvailable(f) == 0 {
                continue;
            }
            // DWORD 하나가 담겨 온다 — 0이면 "하지 마".
            if let Some(bytes) = read_hglobal(f) {
                if bytes.len() >= 4
                    && u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == 0
                {
                    return true;
                }
            }
        }
    }
    false
}

// ───────────────────────────── 읽기

/// `HGLOBAL` 포맷의 날바이트. 실패하면 `None`(빈 벡터와 구분한다).
///
/// # Safety
/// 클립보드가 열려 있어야 한다.
unsafe fn read_hglobal(fmt: u32) -> Option<Vec<u8>> {
    unsafe {
        let h = GetClipboardData(fmt);
        if h == NULL_HANDLE {
            return None;
        }
        let size = GlobalSize(h);
        if size == 0 {
            return Some(Vec::new());
        }
        let p = GlobalLock(h);
        if p.is_null() {
            return None;
        }
        let out = core::slice::from_raw_parts(p.cast::<u8>(), size).to_vec();
        GlobalUnlock(h);
        Some(out)
    }
}

/// 클립보드를 연다 — ★ **다른 앱이 잡고 있으면 짧게 물러섰다 다시 시도**한다.
///
/// 복사 직후에는 복사한 앱이 아직 쥐고 있는 일이 흔하다. 한 번 실패하고 포기하면
/// *"가끔 복사가 안 잡힌다"* 가 된다.
fn open_with_retry() -> bool {
    for attempt in 0..10 {
        // SAFETY: 인자 없는 단순 호출.
        if unsafe { OpenClipboard(NULL_HANDLE) } != 0 {
            return true;
        }
        // 5·10·15… ms — 합쳐도 275ms를 넘지 않는다(팝업 예산 밖의 백그라운드 작업).
        unsafe { Sleep(5 * (attempt + 1)) };
    }
    false
}

/// 클립보드 주인 프로세스의 **실행 파일 이름**(확장자 뺀 것). 실패하면 `None`.
fn owner_app() -> Option<String> {
    // SAFETY: 실패는 전부 널/0으로 돌아오고 그때마다 빠져나간다.
    unsafe {
        let hwnd = GetClipboardOwner();
        if hwnd == NULL_HANDLE {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == NULL_HANDLE {
            return None;
        }
        let mut buf = [0u16; 260];
        let mut len: u32 = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let path = from_wide(&buf[..len as usize]);
        // 경로 전체가 아니라 **앱 이름만** — 목록 배지에 경로를 띄우면 길고 사생활이다.
        let name = path.rsplit(['\\', '/']).next()?;
        Some(
            name.trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_string(),
        )
    }
}

/// 지금 클립보드를 한 벌 읽는다. 열지 못하면 `None`.
///
/// ★ **`WM_CLIPBOARDUPDATE` 없이도 부를 수 있다** — 첫 실행 때 이미 있던 내용을 담거나,
/// 진단(`spike-watch`)에서 한 번만 볼 때 쓴다.
#[must_use]
pub fn read_snapshot() -> Option<ClipSnapshot> {
    if !open_with_retry() {
        return None;
    }
    // SAFETY: 위에서 열었고, 어떤 경로로 빠져나가든 아래에서 반드시 닫는다.
    let snap = unsafe {
        let seq = u64::from(GetClipboardSequenceNumber());
        let concealed = read_concealed();
        let mut reps = Vec::new();
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            let name = standard_name(fmt).map_or_else(
                || {
                    let mut buf = [0u16; 256];
                    let n = GetClipboardFormatNameW(fmt, buf.as_mut_ptr(), buf.len() as i32);
                    if n > 0 {
                        from_wide(&buf[..n as usize])
                    } else {
                        // 이름을 못 얻는 등록 포맷 — 번호로라도 **구분은 되게** 남긴다.
                        format!("CF_{fmt}")
                    }
                },
                str::to_string,
            );
            // ⚠️ 핸들 포맷은 바이트를 읽지 않는다 — 이름만 담는다.
            let data = if is_handle_format(fmt) {
                Vec::new()
            } else {
                read_hglobal(fmt).unwrap_or_default()
            };
            reps.push(RawRep { format: name, data });
            fmt = EnumClipboardFormats(fmt);
        }
        ClipSnapshot {
            reps,
            source_app: owner_app(),
            concealed,
            seq,
        }
    };
    // SAFETY: 짝 맞춘 닫기. 안 닫으면 **다른 앱이 클립보드를 못 쓴다**.
    unsafe { CloseClipboard() };
    Some(snap)
}

// ───────────────────────────── 감시 루프

thread_local! {
    /// 이 스레드의 콜백 — 창 프로시저가 `WM_CLIPBOARDUPDATE`에서 부른다.
    static SINK: std::cell::RefCell<Option<Sink>> = const { std::cell::RefCell::new(None) };
    /// 직전에 처리한 일련번호 — ★ **같은 변화를 두 번 받는 것을 막는다**.
    static LAST_SEQ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// 남은 재시도 횟수([`RETRY_MAX`]부터 줄어든다).
    static RETRIES_LEFT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `NEXA_CLIP_DIAG=1`일 때만 stderr로 진단을 찍는다 — 실기에서 *"이벤트가 안 온다"* 와
/// *"와서 버려진다"* 를 구분하는 용도(08-27 탐색기 복사 유실 조사).
fn diag(msg: &str) {
    if std::env::var_os("NEXA_CLIP_DIAG").is_some() {
        eprintln!("[diag] {msg}");
    }
}

/// 스냅숏을 읽어 콜백에 넘긴다. **클립보드를 열지 못했으면 `false`**.
///
/// 읽기는 됐지만 중복 일련번호라 건너뛴 경우는 `true`다 — 재시도할 일이 아니다.
fn try_deliver() -> bool {
    let Some(snap) = read_snapshot() else {
        diag("클립보드 열기 실패");
        return false;
    };
    // ★ **전이 중간의 빈 스냅숏은 "처리했다"고 치면 안 된다**(08-27 실기 ⑫).
    //
    // 탐색기는 비우기와 채우기를 별개 트랜잭션으로 한다. 그 틈을 읽으면 표현 0개인데
    // 일련번호는 이미 최종값이고, **이후 채우기는 번호를 더 올리지 않는다**(지연 렌더링 경로).
    // 여기서 `LAST_SEQ`를 갱신해 버리면 하트비트도 이벤트도 그 복사를 **영영 못 본다** —
    // 실패로 돌려 재시도 타이머(200ms)가 채워진 뒤를 다시 읽게 한다.
    if !nclip_core::capture::has_content(&snap.reps) {
        diag(&format!("내용 없는 스냅숏(seq {}) — 재시도", snap.seq));
        return false;
    }
    // ⚠️ 같은 일련번호가 두 번 오는 일이 있다(리스너 중복 등록·앱의 재게시).
    //    걸러 내지 않으면 목록에 같은 항목이 쌍으로 쌓인다.
    let dup = LAST_SEQ.with(|c| {
        let same = snap.seq != 0 && c.get() == snap.seq;
        c.set(snap.seq);
        same
    });
    if dup {
        diag(&format!("seq 중복({}) — 건너뜀", snap.seq));
    } else {
        diag(&format!("전달 seq={} 표현 {}개", snap.seq, snap.reps.len()));
        SINK.with(|s| {
            if let Some(f) = s.borrow().as_ref() {
                f(snap);
            }
        });
    }
    true
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        diag("WM_CLIPBOARDUPDATE 수신");
        // ⚠️ **실패를 조용히 버리면 안 된다** — 탐색기 복사는 비동기 플러시(`AsyncFlag`)
        //    동안 클립보드를 계속 쥐고 있어 [`open_with_retry`]의 275ms를 넘기기도 한다.
        //    그리고 `WM_CLIPBOARDUPDATE`는 **병합**되므로 다시 오지 않는다 —
        //    여기서 놓치면 *"복사가 안 잡혔다"* 로 끝난다(08-27 실기 2차).
        // SAFETY: 방금 받은 창 핸들로 타이머를 걸고 끈다.
        unsafe {
            if try_deliver() {
                KillTimer(hwnd, RETRY_TIMER_ID);
                RETRIES_LEFT.with(|c| c.set(0));
            } else {
                RETRIES_LEFT.with(|c| c.set(RETRY_MAX));
                SetTimer(hwnd, RETRY_TIMER_ID, RETRY_MS, 0);
            }
        }
        return 0;
    }
    if msg == WM_TIMER && w == RETRY_TIMER_ID {
        let left = RETRIES_LEFT.with(|c| c.get());
        // SAFETY: 위와 같은 창 핸들.
        unsafe {
            if try_deliver() || left <= 1 {
                KillTimer(hwnd, RETRY_TIMER_ID);
                RETRIES_LEFT.with(|c| c.set(0));
            } else {
                RETRIES_LEFT.with(|c| c.set(left - 1));
            }
        }
        return 0;
    }
    if msg == WM_TIMER && w == POLL_TIMER_ID {
        // ★ 안전망 — 일련번호가 움직였는데 이벤트를 못 받았으면 여기서 잡는다.
        //   이벤트 경로가 이미 처리했으면 번호가 같아서 아무 일도 안 한다(중복 없음).
        // SAFETY: 인자 없는 조회 호출.
        let seq = u64::from(unsafe { GetClipboardSequenceNumber() });
        if seq != 0 && LAST_SEQ.with(|c| c.get()) != seq {
            diag(&format!("하트비트 — 놓친 변화 감지(seq {seq})"));
            // 열기 실패·빈 스냅숏이면 그냥 둔다 — 다음 하트비트가 다시 본다.
            let _ = try_deliver();
        }
        return 0;
    }
    if msg == WM_DESTROY {
        return 0;
    }
    // SAFETY: 나머지는 OS 기본 처리로 넘긴다.
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

/// 감시 시작 — **전용 스레드**에 메시지 전용 창을 띄우고 그 안에서 돈다.
///
/// # Errors
/// 창을 만들지 못하거나 리스너 등록이 실패하면 [`WatchError::Os`].
pub fn start(on_change: Sink) -> Result<(), WatchError> {
    // ★ 스레드 안에서 창을 만들고, **성공/실패를 여기로 되돌려 받는다**.
    //   안 그러면 "start는 Ok인데 실제로는 아무 일도 안 일어나는" 상태가 된다.
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    let spawned = std::thread::Builder::new()
        .name("nclip-clipboard".into())
        .spawn(move || {
            let cls = wide("NexaClipWatch");
            // SAFETY: 표준 창 등록·생성 순서. 실패는 0/널로 돌아온다.
            let hwnd = unsafe {
                let h_instance = GetModuleHandleW(core::ptr::null());
                let wc = WNDCLASSW {
                    style: 0,
                    lpfn_wnd_proc: Some(wnd_proc),
                    cb_cls_extra: 0,
                    cb_wnd_extra: 0,
                    h_instance,
                    h_icon: NULL_HANDLE,
                    h_cursor: NULL_HANDLE,
                    hbr_background: NULL_HANDLE,
                    lpsz_menu_name: core::ptr::null(),
                    lpsz_class_name: cls.as_ptr(),
                };
                // 이미 등록돼 있으면 0이 돌아오지만 그대로 진행해도 된다.
                RegisterClassW(&wc);
                CreateWindowExW(
                    0,
                    cls.as_ptr(),
                    cls.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE, // ★ 화면에 뜨지 않는 메시지 전용 창
                    NULL_HANDLE,
                    h_instance,
                    core::ptr::null(),
                )
            };
            if hwnd == NULL_HANDLE {
                let _ = tx.send(Err("메시지 창 생성 실패".into()));
                return;
            }
            // SAFETY: 방금 만든 창에 리스너를 붙인다.
            if unsafe { AddClipboardFormatListener(hwnd) } == 0 {
                let _ = tx.send(Err("AddClipboardFormatListener 실패".into()));
                return;
            }
            // ★ 하트비트 기준점 — 지금 있는 내용을 "이미 본 것"으로 친다.
            //   안 하면 첫 하트비트가 기존 클립보드를 새 항목처럼 배달한다
            //   (`read_now`가 이미 보여 준 것의 중복).
            // SAFETY: 조회 + 타이머 등록.
            unsafe {
                LAST_SEQ.with(|c| c.set(u64::from(GetClipboardSequenceNumber())));
                SetTimer(hwnd, POLL_TIMER_ID, POLL_MS, 0);
            }
            SINK.with(|s| *s.borrow_mut() = Some(on_change));
            let _ = tx.send(Ok(()));

            // 메시지 펌프 — `GetMessageW`가 블록하므로 **유휴에서 CPU를 쓰지 않는다**.
            let mut msg = MSG {
                hwnd: NULL_HANDLE,
                message: 0,
                w_param: 0,
                l_param: 0,
                time: 0,
                pt_x: 0,
                pt_y: 0,
            };
            // SAFETY: 표준 메시지 루프. 0(WM_QUIT) 또는 -1(오류)에서 끝난다.
            unsafe {
                while GetMessageW(&mut msg, NULL_HANDLE, 0, 0) > 0 {
                    DispatchMessageW(&msg);
                }
            }
        })
        .map_err(|e| WatchError::Os(format!("감시 스레드 생성 실패: {e}")))?;
    drop(spawned);

    // 창이 실제로 서는 것을 확인하고 나서 Ok를 준다.
    match rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(WatchError::Os(e)),
        Err(_) => Err(WatchError::Os("감시 스레드가 응답하지 않습니다".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 표준 번호에 이름이 붙는다 — ★ 없으면 **전부 "벤더"로 분류**된다.
    #[test]
    fn standard_formats_have_names() {
        assert_eq!(standard_name(13), Some("CF_UNICODETEXT"));
        assert_eq!(standard_name(15), Some("CF_HDROP"));
        assert_eq!(standard_name(8), Some("CF_DIB"));
        assert_eq!(standard_name(17), Some("CF_DIBV5"));
        assert_eq!(standard_name(49_999), None, "등록 포맷은 이름표에 없다");
    }

    /// ⚠️ 핸들 포맷을 `GlobalLock`으로 읽으면 안 된다 — 목록을 못 박는다.
    #[test]
    fn handle_formats_are_excluded_from_byte_read() {
        for f in [2, 3, 9, 14] {
            assert!(is_handle_format(f), "{f}는 핸들 포맷이다");
        }
        for f in [1, 8, 13, 15, 17] {
            assert!(!is_handle_format(f), "{f}는 HGLOBAL이다");
        }
    }

    /// 표준 이름이 [`nclip_core::capture`]의 판정과 **맞물린다**.
    ///
    /// ★ 이름표와 분류표가 따로 놀면 `CF_DIB`를 읽어 놓고도 이미지로 못 알아본다.
    #[test]
    fn names_line_up_with_classifier() {
        use nclip_core::capture::{is_bitmap_format, is_files_format};
        assert!(is_bitmap_format(standard_name(8).unwrap()));
        assert!(is_bitmap_format(standard_name(17).unwrap()));
        assert!(is_files_format(standard_name(15).unwrap()));
        assert!(nclip_core::is_plain_format(standard_name(13).unwrap()));
    }

    #[test]
    fn wide_round_trips() {
        let w = wide("CF_HTML");
        assert_eq!(from_wide(&w), "CF_HTML");
        assert_eq!(*w.last().unwrap(), 0, "널 종단이 있어야 한다");
    }

    #[test]
    fn from_wide_stops_at_nul() {
        let buf = [65u16, 66, 0, 67];
        assert_eq!(from_wide(&buf), "AB");
    }
}
