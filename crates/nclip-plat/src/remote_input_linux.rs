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
        let _ = std::fs::write(p, t);
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

    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from("nclip_rd_create"));
    o.insert("session_handle_token", Value::from("nclip_rd"));
    let (code, r) = call_with_response(&conn, &portal, "CreateSession", &(o,), "nclip_rd_create")?;
    if code != 0 {
        return Err(zbus::Error::Unsupported);
    }
    let path: OwnedObjectPath = r
        .get("session_handle")
        .and_then(|v| String::try_from(v.clone()).ok())
        .and_then(|s| OwnedObjectPath::try_from(s).ok())
        .ok_or(zbus::Error::Unsupported)?;

    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from("nclip_rd_select"));
    o.insert("types", Value::from(DEVICE_KEYBOARD));
    o.insert("persist_mode", Value::from(PERSIST_UNTIL_REVOKED));
    let saved = load_token();
    if let Some(t) = &saved {
        o.insert("restore_token", Value::from(t.as_str()));
    }
    let (code, _) = call_with_response(
        &conn,
        &portal,
        "SelectDevices",
        &(&path, o),
        "nclip_rd_select",
    )?;
    if code != 0 {
        return Err(zbus::Error::Unsupported);
    }

    // 첫 회 = 셸 대화창(사용자 승인). 토큰이 있으면 대화창 없이 통과한다.
    let mut o: HashMap<&str, Value<'_>> = HashMap::new();
    o.insert("handle_token", Value::from("nclip_rd_start"));
    let (code, r) = call_with_response(&conn, &portal, "Start", &(&path, "", o), "nclip_rd_start")?;
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

/// `Ctrl+V` 한 번 — 세션이 없으면 만든다(대화창 가능).
///
/// # Errors
/// 세션 실패 · 전송 실패.
pub fn tap_ctrl_v() -> Result<(), String> {
    ensure_session()?;
    let g = SESSION.lock().map_err(|_| "세션 잠금 오염".to_string())?;
    let s = g.as_ref().ok_or_else(|| "세션 없음".to_string())?;
    let run = || -> zbus::Result<()> {
        key(s, KEY_LEFTCTRL, true)?;
        key(s, KEY_V, true)?;
        key(s, KEY_V, false)?;
        key(s, KEY_LEFTCTRL, false)
    };
    run().map_err(|e| format!("NotifyKeyboardKeycode 실패: {e}"))
}

/// 임의 키 한 번(누름+뗌) — 진단·실기 전용(evdev 키코드).
///
/// # Errors
/// 세션 실패 · 전송 실패.
pub fn tap_key(code: i32, with_ctrl: bool) -> Result<(), String> {
    ensure_session()?;
    let g = SESSION.lock().map_err(|_| "세션 잠금 오염".to_string())?;
    let s = g.as_ref().ok_or_else(|| "세션 없음".to_string())?;
    let run = || -> zbus::Result<()> {
        if with_ctrl {
            key(s, KEY_LEFTCTRL, true)?;
        }
        key(s, code, true)?;
        key(s, code, false)?;
        if with_ctrl {
            key(s, KEY_LEFTCTRL, false)?;
        }
        Ok(())
    };
    run().map_err(|e| format!("NotifyKeyboardKeycode 실패: {e}"))
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
