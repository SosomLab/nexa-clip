//! Wayland 키 주입 — **xdg 포털 `RemoteDesktop`**(K-1 Linux · 08-30 실기가 잡음).
//!
//! 실측: 이 PC의 Xwayland는 `-enable-ei-portal`로 떠 있다 — XTest는 X 서버에서 `ok`로 끝나도
//! Wayland 앱에는 **포털 권한을 거쳐야** 전달된다(libei 경로). 그래서 XTest 대신 우리가 직접
//! `org.freedesktop.portal.RemoteDesktop`(v2)에 세션을 열고 `NotifyKeyboardKeycode`로 evdev
//! 키코드를 넣는다 — GNOME·KDE 공통의 정식 Wayland 통로다.
//!
//! 권한: 첫 `Start`에서 셸이 **"원격 데스크톱 허용" 대화창**을 띄운다. `persist_mode=2`(영구)로
//! 받은 `restore_token`을 파일에 두면 다음 실행부터 대화창이 없다. 거부 = 정직한 실패
//! (호스트는 클립보드 적재까지만 — FR-P-1).
//!
//! 세션은 프로세스 수명 동안 하나(연결 유지 필수 — 연결이 닫히면 세션도 닫힌다).
//!
//! ★ 세션 닫힘 자가 복구(09-05 사용자 실기 — Linux VM): 셸이 세션을 닫을 수 있다(상단 바 "화면 공유"
//! 표시의 중지 · 포털 재시작 · 잠금 등). 그러면 `NotifyKeyboardKeycode`가 `AccessDenied: Invalid
//! session`으로 **영영** 실패했다(죽은 핸들을 계속 씀). 이제 전송 실패 시 세션을 버리고 저장 토큰으로
//! 다시 열어 **한 번 재시도**한다 — 토큰이 살아 있으면 대화창 없이 조용히 붙는다(토큰이 회수됐으면
//! 대화창 = 정직한 재승인).

use crate::hotkey_linux::{call_with_response, Results, PORTAL_DEST, PORTAL_PATH};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, Value};

const IFACE: &str = "org.freedesktop.portal.RemoteDesktop";
/// evdev 키코드(`linux/input-event-codes.h`).
pub const KEY_LEFTCTRL: i32 = 29;
pub const KEY_V: i32 = 47;
const DEVICE_KEYBOARD: u32 = 1;
const PERSIST_UNTIL_REVOKED: u32 = 2;

struct Session {
    conn: Connection,
    path: OwnedObjectPath,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static TOKEN_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 세션 회차 — 포털 객체 경로는 `session_handle_token`에서 파생되므로 재개설마다 달라야 한다
/// (같은 토큰으로 다시 만들면 죽은 옛 객체와 경로가 겹칠 수 있다).
static GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// `restore_token` 보관 파일(호스트의 data 디렉터리) — 설정 전엔 토큰을 저장하지 않는다
/// (매 실행 대화창).
pub fn configure_token_path(p: PathBuf) {
    let _ = TOKEN_PATH.set(p);
}

/// 포털 `RemoteDesktop`이 세션 버스에 있는가(대화창 없음).
#[must_use]
pub fn available() -> bool {
    let Ok(conn) = Connection::session() else {
        return false;
    };
    Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE)
        .and_then(|p| p.get_property::<u32>("version"))
        .is_ok()
}

fn load_token() -> Option<String> {
    let p = TOKEN_PATH.get()?;
    let t = std::fs::read_to_string(p).ok()?;
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn save_token(t: &str) {
    if let Some(p) = TOKEN_PATH.get() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // ★ 소유자만(09-05) — 이 토큰은 대화창 없이 키 주입 세션을 여는 열쇠다.
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(p)
            .and_then(|mut f| f.write_all(t.as_bytes()));
    }
}

/// 세션 확보 — 없으면 만든다(첫 회 대화창). 이미 있으면 즉시 Ok.
///
/// # Errors
/// 포털 부재 · 사용자 거부 · 버스 오류(사유 문자열).
pub fn ensure_session() -> Result<(), String> {
    let mut g = SESSION.lock().map_err(|_| "세션 잠금 오염".to_string())?;
    if g.is_some() {
        return Ok(());
    }
    let s = open_session().map_err(|e| format!("포털 RemoteDesktop: {e}"))?;
    *g = Some(s);
    Ok(())
}

fn open_session() -> zbus::Result<Session> {
    let conn = Connection::session()?;
    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE)?;
    let _v: u32 = portal.get_property("version")?;

    let gen = GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tok_create = format!("nclip_rd_create{gen}");
    let tok_session = format!("nclip_rd{gen}");
    let tok_select = format!("nclip_rd_select{gen}");
    let tok_start = format!("nclip_rd_start{gen}");
    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from(tok_create.as_str()));
    o.insert("session_handle_token", Value::from(tok_session.as_str()));
    let (code, r) = call_with_response(&conn, &portal, "CreateSession", &(o,), &tok_create)?;
    if code != 0 {
        return Err(zbus::Error::Unsupported);
    }
    let path: OwnedObjectPath = r
        .get("session_handle")
        .and_then(|v| String::try_from(v.clone()).ok())
        .and_then(|s| OwnedObjectPath::try_from(s).ok())
        .ok_or(zbus::Error::Unsupported)?;

    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from(tok_select.as_str()));
    o.insert("types", Value::from(DEVICE_KEYBOARD));
    o.insert("persist_mode", Value::from(PERSIST_UNTIL_REVOKED));
    let saved = load_token();
    if let Some(t) = &saved {
        o.insert("restore_token", Value::from(t.as_str()));
    }
    let (code, _) = call_with_response(&conn, &portal, "SelectDevices", &(&path, o), &tok_select)?;
    if code != 0 {
        return Err(zbus::Error::Unsupported);
    }

    // 첫 회 = 셸 대화창(사용자 승인). 토큰이 있으면 대화창 없이 통과한다.
    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from(tok_start.as_str()));
    let (code, r) = call_with_response(&conn, &portal, "Start", &(&path, "", o), &tok_start)?;
    if code != 0 {
        return Err(zbus::Error::Unsupported); // 거부/취소
    }
    if let Some(t) = r
        .get("restore_token")
        .and_then(|v| String::try_from(v.clone()).ok())
    {
        save_token(&t);
    }
    Ok(Session { conn, path })
}

fn key(s: &Session, code: i32, pressed: bool) -> zbus::Result<()> {
    let o: HashMap<&str, Value<'_>> = HashMap::new();
    s.conn.call_method(
        Some(PORTAL_DEST),
        PORTAL_PATH,
        Some(IFACE),
        "NotifyKeyboardKeycode",
        &(&s.path, o, code, u32::from(pressed)),
    )?;
    Ok(())
}

/// 죽은 세션 폐기 — 다음 `ensure_session`이 새로 연다. 옛 객체에 `Close`를 시도한다(있으면 정리 · 없어도 무해).
fn drop_session() {
    let Ok(mut g) = SESSION.lock() else {
        return;
    };
    if let Some(s) = g.take() {
        let _ = s.conn.call_method(
            Some(PORTAL_DEST),
            s.path.as_str(),
            Some("org.freedesktop.portal.Session"),
            "Close",
            &(),
        );
    }
}

/// 키 시퀀스를 세션에 보낸다 — 실패하면 세션을 버리고 **한 번** 다시 열어 재시도(★ 09-05 자가 복구).
fn with_session_retry(seq: &dyn Fn(&Session) -> zbus::Result<()>) -> Result<(), String> {
    let once = || -> Result<zbus::Result<()>, String> {
        ensure_session()?;
        let g = SESSION.lock().map_err(|_| "세션 잠금 오염".to_string())?;
        let s = g.as_ref().ok_or_else(|| "세션 없음".to_string())?;
        Ok(seq(s))
    };
    match once()? {
        Ok(()) => Ok(()),
        Err(first) => {
            eprintln!("키 주입: 포털 세션 끊김({first}) — 다시 엽니다");
            drop_session();
            match once()? {
                Ok(()) => Ok(()),
                Err(e) => Err(format!("NotifyKeyboardKeycode 실패: {e}")),
            }
        }
    }
}

/// `Ctrl+V` 한 번 — 세션이 없으면 만든다(대화창 가능). 세션이 죽어 있으면 다시 열어 재시도한다.
///
/// # Errors
/// 세션 실패 · 전송 실패(재시도 후).
pub fn tap_ctrl_v() -> Result<(), String> {
    with_session_retry(&|s| {
        key(s, KEY_LEFTCTRL, true)?;
        key(s, KEY_V, true)?;
        key(s, KEY_V, false)?;
        key(s, KEY_LEFTCTRL, false)
    })
}

/// 임의 키 한 번(누름+뗌) — 진단·실기 전용(evdev 키코드).
///
/// # Errors
/// 세션 실패 · 전송 실패(재시도 후).
pub fn tap_key(code: i32, with_ctrl: bool) -> Result<(), String> {
    with_session_retry(&move |s| {
        if with_ctrl {
            key(s, KEY_LEFTCTRL, true)?;
        }
        key(s, code, true)?;
        key(s, code, false)?;
        if with_ctrl {
            key(s, KEY_LEFTCTRL, false)?;
        }
        Ok(())
    })
}

/// 세션이 살아 있는지(호스트 진단용).
#[must_use]
pub fn has_session() -> bool {
    SESSION.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// 결과 사전에서 문자열 하나(테스트 가능한 순수 헬퍼).
#[allow(dead_code)]
fn get_str(r: &Results, k: &str) -> Option<String> {
    r.get(k).and_then(|v| String::try_from(v.clone()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ 실기 재현(사람 없이) — gnome-text-editor를 띄워 xclip으로 클립보드를 바꿔 가며
    /// 포털 Ctrl+V → Ctrl+S를 **두 번** 넣고 파일을 읽는다(08-30 "첫 번만 붙는다" 재현용).
    /// `NCLIP_RD_TOKEN=<token path> cargo test -p nclip-plat -- --ignored portal_paste_twice --nocapture`
    #[test]
    #[ignore = "데스크톱 세션·gnome-text-editor 필요(수동)"]
    fn portal_paste_twice_into_text_editor() {
        use std::process::{Command, Stdio};
        if let Some(t) = std::env::var_os("NCLIP_RD_TOKEN") {
            configure_token_path(t.into());
        }
        let dir = std::env::temp_dir().join(format!("nclip-rd-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.txt");
        std::fs::write(&file, "").unwrap();
        let mut ed = Command::new("gnome-text-editor")
            .arg(&file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("editor");
        std::thread::sleep(std::time::Duration::from_millis(2500));
        for text in ["ONE", "TWO"] {
            let mut c = Command::new("xclip")
                .args([
                    "-selection",
                    "clipboard",
                    "-t",
                    "text/plain;charset=utf-8",
                    "-i",
                ])
                .stdin(Stdio::piped())
                .spawn()
                .expect("xclip");
            use std::io::Write as _;
            c.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
            c.wait().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(400));
            tap_ctrl_v().expect("ctrl+v");
            std::thread::sleep(std::time::Duration::from_millis(400));
            tap_key(31, true).expect("ctrl+s"); // KEY_S
            std::thread::sleep(std::time::Duration::from_millis(800));
            let got = std::fs::read_to_string(&file).unwrap_or_default();
            eprintln!("after {text}: file = {got:?}");
        }
        let _ = ed.kill();
        let _ = ed.wait();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// ★ 09-05 재현(사람 없이 · 무해) — 세션을 **포털 쪽에서 닫아** 죽은 핸들을 만든 뒤(사용자 실기의
    /// "화면 공유 표시가 사라짐"과 같은 상태) `tap_key`가 세션을 다시 열어 전송에 성공하는지.
    /// 주입 키는 `LeftShift` 단독(어느 창에 가도 아무 일도 안 일어난다) — 포커스 창을 건드리지 않는다.
    /// 09-05 실측: 첫 전송 ✓ → Close → "Invalid session" 감지 → 재개설 → 전송 ✓(대화창 없음 · 토큰 재사용).
    /// `NCLIP_RD_TOKEN=<token path> cargo test -p nclip-plat -- --ignored portal_recovers --nocapture`
    #[test]
    #[ignore = "데스크톱 세션(포털 RemoteDesktop) 필요"]
    fn portal_recovers_after_session_closed() {
        const KEY_LEFTSHIFT: i32 = 42;
        if let Some(t) = std::env::var_os("NCLIP_RD_TOKEN") {
            configure_token_path(t.into());
        }
        tap_key(KEY_LEFTSHIFT, false).expect("첫 전송");
        let gen_before = GEN.load(std::sync::atomic::Ordering::Relaxed);
        // ★ 세션 사살 — 핸들은 그대로 둔 채 포털에 Close(= 셸이 세션을 닫은 것과 같은 상태).
        {
            let g = SESSION.lock().unwrap();
            let s = g.as_ref().expect("세션");
            s.conn
                .call_method(
                    Some(PORTAL_DEST),
                    s.path.as_str(),
                    Some("org.freedesktop.portal.Session"),
                    "Close",
                    &(),
                )
                .expect("Close");
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        tap_key(KEY_LEFTSHIFT, false).expect("복구 후 전송");
        let gen_after = GEN.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            gen_after,
            gen_before + 1,
            "세션을 정확히 한 번 다시 열어야 한다"
        );
        assert!(has_session());
    }

    /// 토큰 파일 왕복 — 공백은 걷어내고, 비어 있으면 None.
    #[test]
    fn token_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("nclip-rd-{}", std::process::id()));
        let p = dir.join("rd.token");
        configure_token_path(p);
        // OnceLock이라 다른 테스트가 먼저 잡았을 수 있다 — 실제 경로로 검증한다.
        let Some(real) = TOKEN_PATH.get() else { return };
        save_token(" abc \n");
        assert_eq!(std::fs::read_to_string(real).unwrap().trim(), "abc");
        assert_eq!(load_token().as_deref(), Some("abc"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
