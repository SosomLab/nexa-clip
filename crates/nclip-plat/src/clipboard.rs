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

/// Linux — **도구 파이프 1단**(08-30): `wl-copy`(Wayland) / `xclip`(X11)에 stdin으로 넘긴다.
/// 읽기([`crate::watch_linux`])와 같은 도구·같은 판별(`WAYLAND_DISPLAY` 우선 · PATH 확인).
///
/// ⚠️ **표현 한 개만 게시한다** — 두 도구 모두 `--type`/`-t` 하나뿐이다(Wayland에서
/// 다중 표현은 자체 `wl_data_source`가 필요하고, 그건 **입력 시리얼을 가진 표면**이 있어야
/// `set_selection`이 받아들여진다 — 팝업 창의 시리얼을 빌리는 후속 과제). 대표 표현은
/// [`pick_rep`]가 고른다: 파일(`text/uri-list`) > 이미지(`image/png`) > 평문 > HTML > 첫 것.
/// 게시 수는 그래서 최대 1이다 — 호스트가 "표현 N개"라고 찍는 값이 곧 사실이다.
#[cfg(target_os = "linux")]
mod imp {
    use super::RawRep;
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    /// 표현 우선순위 — 붙여넣기 판단이 가장 잘 되는 것부터.
    fn rank(format: &str) -> u8 {
        match format {
            "text/uri-list" => 0,
            "image/png" => 1,
            "text/plain" => 2,
            "text/html" => 3,
            "image/jpeg" | "image/bmp" => 4,
            _ => 9,
        }
    }

    /// 게시할 대표 표현 하나(날바이트가 있는 것 중 우선순위 최상) — 없으면 None.
    pub(super) fn pick_rep(reps: &[RawRep]) -> Option<&RawRep> {
        reps.iter()
            .filter(|r| !r.data.is_empty())
            .min_by_key(|r| (rank(&r.format), r.format.len()))
    }

    /// 도구에 넘길 MIME — 판정 어휘의 `text/plain`은 UTF-8을 명시해 준다(GTK가 charset
    /// 없는 `text/plain`을 Latin-1로 읽는 사고 방지).
    fn wire_type(format: &str) -> String {
        if format == "text/plain" {
            "text/plain;charset=utf-8".into()
        } else {
            format.to_string()
        }
    }

    fn tool_exists(cmd: &str) -> bool {
        Command::new(cmd)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    pub(super) fn set_reps(reps: &[RawRep]) -> Result<usize, String> {
        let rep = pick_rep(reps).ok_or_else(|| "게시할 표현이 없습니다".to_string())?;
        let ty = wire_type(&rep.format);
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let x11 = std::env::var_os("DISPLAY").is_some();
        let (cmd, args): (&str, Vec<&str>) = if wayland && tool_exists("wl-copy") {
            ("wl-copy", vec!["--type", &ty])
        } else if x11 && tool_exists("xclip") {
            ("xclip", vec!["-selection", "clipboard", "-t", &ty, "-i"])
        } else if !wayland && !x11 {
            return Err("표시 서버가 없습니다(WAYLAND_DISPLAY/DISPLAY 없음)".into());
        } else {
            // 읽기(`watch_linux`)와 같은 도구라 안내도 같다 — 설치 명령은 `nexa-clip watch`가 배포판별로.
            return Err(format!(
                "클립보드 쓰기 도구가 없습니다 — {} (감시와 같은 도구 · `nexa-clip watch`의 조치 안내 참조)",
                if wayland { "wl-clipboard (wl-copy)" } else { "xclip" }
            ));
        };
        // 두 도구 모두 셀렉션 서빙을 위해 스스로 분리(fork)된다 — 우리는 stdin을 닫고
        // 상위 프로세스 종료만 기다린다(블로킹 없음).
        let mut child = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{cmd} 실행 실패: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&rep.data)
                .map_err(|e| format!("{cmd} 입력 실패: {e}"))?;
        }
        let st = child.wait().map_err(|e| format!("{cmd} 대기 실패: {e}"))?;
        if !st.success() {
            return Err(format!("{cmd} 종료 코드 {st}"));
        }
        Ok(1)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn rep(f: &str, d: &[u8]) -> RawRep {
            RawRep {
                format: f.into(),
                data: d.to_vec(),
            }
        }

        /// 파일 > 이미지 > 평문 > HTML — 빈 바이트는 후보가 아니다.
        #[test]
        fn pick_rep_prefers_files_then_image_then_plain() {
            let reps = [
                rep("text/html", b"<b>x</b>"),
                rep("text/plain", b"x"),
                rep("image/png", b""),
            ];
            assert_eq!(
                pick_rep(&reps).map(|r| r.format.as_str()),
                Some("text/plain")
            );
            let reps = [rep("text/plain", b"p"), rep("text/uri-list", b"file:///a")];
            assert_eq!(
                pick_rep(&reps).map(|r| r.format.as_str()),
                Some("text/uri-list")
            );
            assert!(pick_rep(&[rep("image/png", b"")]).is_none());
        }

        #[test]
        fn wire_type_adds_utf8_to_plain() {
            assert_eq!(wire_type("text/plain"), "text/plain;charset=utf-8");
            assert_eq!(wire_type("image/png"), "image/png");
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
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
