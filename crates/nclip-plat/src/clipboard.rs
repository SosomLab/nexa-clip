//! 클립보드 **쓰기**(재적재) — 이력 항목의 표현 전부를 되돌린다(P-2 · 원본 붙여넣기).
//!
//! 읽기(감시)는 [`crate::watch_win`]에 있다. 여기는 반대 방향 — 트레이/팝업에서 항목을
//! 고르면 **가진 표현을 이름째 전부** 게시해 붙여넣기 판단을 받는 앱에 넘긴다
//! ([docs/27 §8](../../../docs/27-capture-cases.md)).
//!
//! ## ⚠️ 클립보드는 **실제 HWND로 연다** (beep 08-21 힙 오염 실측 이식)
//!
//! `OpenClipboard(NULL)`은 오픈 상태를 "연 창 핸들"로 대조하는데 NULL==NULL이라
//! **둘째 호출자도 성공**한다 — 그 순간 한쪽 `EmptyClipboard`가 상대가 `GlobalLock` 중인
//! HGLOBAL을 해제해 use-after-free가 된다. 호출마다 메시지 전용 창을 만들어 넘기면
//! 오픈이 진짜로 배타된다. 프로세스 안 스레드끼리는 뮤텍스로 먼저 직렬화한다.
//! (감시의 읽기 경로는 짧고 열람뿐이라 기존 유지 — 쓰기는 `EmptyClipboard`를 하므로 필수.)
//!
//! 이식 원본: `nexa-beep` `crates/nbeep-plat/src/clipboard.rs`(ClipGuard · set 경로).

use nclip_core::RawRep;

/// 표현 묶음을 클립보드에 게시한다. 성공 시 **게시한 표현 수**.
///
/// 날바이트가 없는 표현(핸들 포맷 — `CF_BITMAP` 등)은 건너뛴다(가진 게 없다).
///
/// # Errors
/// 클립보드를 열지 못했거나 게시할 표현이 하나도 없으면 사유 문자열.
pub fn set_reps(reps: &[RawRep]) -> Result<usize, String> {
    imp::set_reps(reps)
}

#[cfg(windows)]
// Win32 타입 이름은 원문 그대로(MSDN 대조) — FFI 선언부에 한해 린트를 끈다.
#[allow(clippy::upper_case_acronyms)]
mod imp {
    use super::RawRep;
    use crate::win32::CreateWindowExW;

    type HANDLE = isize;
    const GMEM_MOVEABLE: u32 = 0x0002;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: HANDLE) -> HANDLE;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
        fn DestroyWindow(hwnd: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> HANDLE;
        fn GlobalLock(h: HANDLE) -> *mut core::ffi::c_void;
        fn GlobalUnlock(h: HANDLE) -> i32;
        fn GlobalFree(h: HANDLE) -> HANDLE;
    }

    /// 표준 포맷 **이름 → 번호**([`crate::watch_win`] 이름표의 역방향 — 같은 17개).
    fn standard_id(name: &str) -> Option<u32> {
        Some(match name {
            "CF_TEXT" => 1,
            "CF_BITMAP" => 2,
            "CF_METAFILEPICT" => 3,
            "CF_SYLK" => 4,
            "CF_DIF" => 5,
            "CF_TIFF" => 6,
            "CF_OEMTEXT" => 7,
            "CF_DIB" => 8,
            "CF_PALETTE" => 9,
            "CF_PENDATA" => 10,
            "CF_RIFF" => 11,
            "CF_WAVE" => 12,
            "CF_UNICODETEXT" => 13,
            "CF_ENHMETAFILE" => 14,
            "CF_HDROP" => 15,
            "CF_LOCALE" => 16,
            "CF_DIBV5" => 17,
            _ => return None,
        })
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 배타 오픈 가드 — 실제(메시지 전용) 창 + 프로세스 내 직렬화.
    struct ClipGuard {
        hwnd: isize,
        _serial: std::sync::MutexGuard<'static, ()>,
    }

    fn open_clipboard() -> Option<ClipGuard> {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let class = wide("STATIC");
        const HWND_MESSAGE: isize = -3;
        // SAFETY: 사전 정의 클래스("STATIC")의 메시지 전용 창 — 표시되지 않고
        // 이 스레드가 만들었다가 Drop에서 같은 스레드로 부순다.
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                core::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                0,
                0,
                core::ptr::null(),
            );
            if hwnd == 0 {
                return None;
            }
            if OpenClipboard(hwnd) == 0 {
                DestroyWindow(hwnd);
                return None;
            }
            Some(ClipGuard {
                hwnd,
                _serial: serial,
            })
        }
    }

    impl Drop for ClipGuard {
        fn drop(&mut self) {
            // SAFETY: open_clipboard에서 연 클립보드/만든 창을 닫는다(짝 보장).
            unsafe {
                CloseClipboard();
                DestroyWindow(self.hwnd);
            }
        }
    }

    pub(super) fn set_reps(reps: &[RawRep]) -> Result<usize, String> {
        let postable: Vec<&RawRep> = reps.iter().filter(|r| !r.data.is_empty()).collect();
        if postable.is_empty() {
            return Err("게시할 표현이 없습니다(핸들 포맷뿐)".into());
        }
        let guard = open_clipboard().ok_or("클립보드를 열지 못했습니다")?;
        let mut posted = 0usize;
        // SAFETY: guard가 배타로 열었다. 각 HGLOBAL은 SetClipboardData 성공 시
        // **시스템 소유**가 된다(해제 금지) — 실패한 것만 우리가 되돌려 해제한다.
        unsafe {
            EmptyClipboard();
            for r in postable {
                let fmt = standard_id(&r.format)
                    .unwrap_or_else(|| RegisterClipboardFormatW(wide(&r.format).as_ptr()));
                if fmt == 0 {
                    continue;
                }
                let h = GlobalAlloc(GMEM_MOVEABLE, r.data.len());
                if h == 0 {
                    continue;
                }
                let p = GlobalLock(h);
                if p.is_null() {
                    GlobalFree(h);
                    continue;
                }
                core::ptr::copy_nonoverlapping(r.data.as_ptr(), p.cast::<u8>(), r.data.len());
                GlobalUnlock(h);
                if SetClipboardData(fmt, h) == 0 {
                    GlobalFree(h);
                } else {
                    posted += 1;
                }
            }
        }
        drop(guard);
        if posted == 0 {
            return Err("표현 게시에 전부 실패했습니다".into());
        }
        Ok(posted)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::RawRep;

    pub(super) fn set_reps(_reps: &[RawRep]) -> Result<usize, String> {
        // 미이식 타깃 — 정직하게 알린다(조용한 무시 금지).
        Err("이 OS의 클립보드 쓰기는 아직 미이식입니다".into())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// ★ 왕복 — 게시한 표현(표준 + 등록 포맷)이 감시 읽기로 그대로 돌아온다.
    ///
    /// 실제 클립보드를 만지므로 기본 실행에서는 건너뛴다(CI·병렬 테스트 오염 방지).
    /// 수동: `cargo test -p nclip-plat -- --ignored clipboard`
    #[test]
    #[ignore = "실제 클립보드를 사용(수동 실행 전용)"]
    fn set_reps_round_trips_through_read() {
        let reps = vec![
            RawRep {
                format: "CF_UNICODETEXT".into(),
                data: "왕복\0".encode_utf16().flat_map(u16::to_le_bytes).collect(),
            },
            RawRep {
                format: "NexaClipTest".into(),
                data: b"vendor-bytes".to_vec(),
            },
        ];
        let n = set_reps(&reps).expect("게시");
        assert_eq!(n, 2);
        let snap = crate::watch_win::read_snapshot().expect("읽기");
        let get = |f: &str| {
            snap.reps
                .iter()
                .find(|r| r.format == f)
                .unwrap_or_else(|| panic!("{f}가 되돌아와야 한다"))
        };
        assert_eq!(snap.plain_text().as_deref(), Some("왕복"));
        assert_eq!(get("NexaClipTest").data, b"vendor-bytes");
    }
}
