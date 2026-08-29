//! 전역 단축키 — Linux(T-15 Linux) = **xdg 데스크톱 포털 `GlobalShortcuts`**.
//!
//! Wayland에는 클라이언트가 전역 키를 잡는 프로토콜이 없다(보안 모델). 표준 통로는
//! `org.freedesktop.portal.GlobalShortcuts`(v1 · GNOME 48+ · KDE 6+)다 — 앱이 원하는
//! 조합("CTRL+SHIFT+v")을 **제안**하면 셸이 사용자에게 확인 대화창을 띄우고(첫 등록 때 ·
//! 사용자가 바꿀 수 있다) 이후 눌릴 때마다 `Activated` 신호를 준다. X11 세션도 같은 포털이
//! 받는다(포털 백엔드가 XGrabKey를 대행). 포털이 없거나 사용자가 거부하면 **정직하게
//! 실패**를 알린다 — 호스트는 트레이 좌클릭 경로를 안내한다.
//!
//! 실측(08-30 · Ubuntu 26.04 GNOME 50): `busctl --user introspect org.freedesktop.portal.Desktop
//! /org/freedesktop/portal/desktop org.freedesktop.portal.GlobalShortcuts` → version 1 ·
//! `BindShortcuts`/`Activated` 있음.
//!
//! 의존 = `zbus`(트레이 SNI와 공유 · 원장 docs/10 §3). 자기 세션 버스 연결을 따로 연다(트레이의
//! object server와 섞이지 않게 — 블로킹 신호 반복자가 그쪽 서빙을 막지 않는다).

use std::collections::HashMap;
use std::sync::Mutex;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// 단축키 이벤트(트레이 어댑터가 [`crate::tray::TrayEvent`]로 옮긴다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// 등록 결과(한 번) — `trigger` = 셸이 확정한 조합의 사람이 읽는 설명(실패 시 빈 문자열).
    Bound { ok: bool, trigger: String },
    /// 눌림(`Activated` — 해제는 무시).
    Activated,
}

pub(crate) const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
pub(crate) const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
/// 단축키 id — 우리 쪽 식별자(셸 대화창엔 `description`이 보인다).
pub const SHORTCUT_ID: &str = "open";
/// 제안 조합(포털 shortcuts-spec 문법) — 설정 `key.open` 기본값 Ctrl+Shift+V.
pub const PREFERRED_TRIGGER: &str = "CTRL+SHIFT+v";

/// 마지막 등록 결과의 조합 설명(호스트 안내용).
static TRIGGER: Mutex<String> = Mutex::new(String::new());

/// 셸이 확정한 조합 설명(등록 전/실패 = 빈 문자열).
#[must_use]
pub fn bound_trigger() -> String {
    TRIGGER.lock().map(|g| g.clone()).unwrap_or_default()
}

/// 포털 등록 + 신호 대기 스레드 기동. 실패도 `Bound { ok: false }`로 **반드시 한 번** 알린다.
pub fn spawn(description: String, on_event: Box<dyn Fn(HotkeyEvent) + Send + Sync>) {
    let _ = std::thread::Builder::new()
        .name("nclip-hotkey".into())
        .spawn(move || match run(&description, &on_event) {
            Ok(()) => {}
            Err(_) => on_event(HotkeyEvent::Bound {
                ok: false,
                trigger: String::new(),
            }),
        });
}

/// 포털 Request/Response 규약 — 응답 신호는 `/…/request/<sender>/<token>` 경로의
/// `org.freedesktop.portal.Request.Response`로 온다. 호출 **전에** 구독해야 안 놓친다.
pub(crate) fn request_path(conn: &Connection, token: &str) -> Option<String> {
    let unique = conn.unique_name()?.to_string();
    let sender = unique.trim_start_matches(':').replace('.', "_");
    Some(format!("{PORTAL_PATH}/request/{sender}/{token}"))
}

pub(crate) type Results = HashMap<String, OwnedValue>;

/// 포털 메서드 호출 → Response(code, results). code 0 = 성공.
pub(crate) fn call_with_response<B>(
    conn: &Connection,
    portal: &Proxy<'_>,
    method: &str,
    body: &B,
    token: &str,
) -> zbus::Result<(u32, Results)>
where
    B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
{
    let path = request_path(conn, token).ok_or(zbus::Error::Unsupported)?;
    let req = Proxy::new(conn, PORTAL_DEST, path, "org.freedesktop.portal.Request")?;
    let mut responses = req.receive_signal("Response")?;
    let _: OwnedObjectPath = portal.call(method, body)?;
    let msg = responses.next().ok_or(zbus::Error::Unsupported)?;
    let (code, results): (u32, Results) = msg.body().deserialize()?;
    Ok((code, results))
}

fn run(description: &str, on_event: &(dyn Fn(HotkeyEvent) + Send + Sync)) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE)?;
    // 포털 부재 = 여기서 실패(version 속성 조회).
    let _version: u32 = portal.get_property("version")?;

    // ① 세션.
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from("nclip_req_session"));
    opts.insert("session_handle_token", Value::from("nclip_session"));
    let (code, results) = call_with_response(
        &conn,
        &portal,
        "CreateSession",
        &(opts,),
        "nclip_req_session",
    )?;
    if code != 0 {
        return Err(zbus::Error::Unsupported);
    }
    let session: OwnedObjectPath = results
        .get("session_handle")
        .and_then(|v| String::try_from(v.clone()).ok())
        .and_then(|s| OwnedObjectPath::try_from(s).ok())
        .ok_or(zbus::Error::Unsupported)?;

    // ② `Activated` 구독 — 등록 **전에**(사용자가 대화창을 닫자마자 누를 수 있다).
    let activated = portal.receive_signal("Activated")?;

    // ③ 등록 — 셸이 사용자 확인 대화창을 띄운다(GNOME). 응답까지 이 스레드는 기다린다.
    let mut sc: HashMap<&str, Value<'_>> = HashMap::new();
    sc.insert("description", Value::from(description));
    sc.insert("preferred_trigger", Value::from(PREFERRED_TRIGGER));
    let shortcuts = vec![(SHORTCUT_ID, sc)];
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from("nclip_req_bind"));
    let (code, results) = call_with_response(
        &conn,
        &portal,
        "BindShortcuts",
        &(&session, shortcuts, "", opts),
        "nclip_req_bind",
    )?;
    let trigger = if code == 0 {
        trigger_description(&results)
    } else {
        String::new()
    };
    if let Ok(mut g) = TRIGGER.lock() {
        *g = trigger.clone();
    }
    on_event(HotkeyEvent::Bound {
        ok: code == 0,
        trigger,
    });
    if code != 0 {
        return Ok(()); // 거부/취소 — 알렸으니 조용히 끝난다.
    }

    // ④ 눌림 대기 — 세션이 살아 있는 동안.
    for msg in activated {
        let Ok((_s, id, _ts, _o)) = msg
            .body()
            .deserialize::<(OwnedObjectPath, String, u64, Results)>()
        else {
            continue;
        };
        if id == SHORTCUT_ID {
            on_event(HotkeyEvent::Activated);
        }
    }
    Ok(())
}

/// `BindShortcuts` 결과의 `shortcuts` a(sa{sv})에서 우리 id의 `trigger_description`.
fn trigger_description(results: &Results) -> String {
    let Some(v) = results.get("shortcuts") else {
        return String::new();
    };
    let Ok(list) = Vec::<(String, HashMap<String, OwnedValue>)>::try_from(v.clone()) else {
        return String::new();
    };
    list.into_iter()
        .find(|(id, _)| id == SHORTCUT_ID)
        .and_then(|(_, props)| props.get("trigger_description").cloned())
        .and_then(|v| String::try_from(v).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 요청 경로 규약 — unique name `:1.42` → `…/request/1_42/<token>`.
    #[test]
    fn request_path_follows_portal_convention() {
        // Connection 없이 규약만 — 같은 변환을 그대로 적용한다.
        let unique = ":1.42";
        let sender = unique.trim_start_matches(':').replace('.', "_");
        assert_eq!(sender, "1_42");
        assert_eq!(
            format!("{PORTAL_PATH}/request/{sender}/tok"),
            "/org/freedesktop/portal/desktop/request/1_42/tok"
        );
    }

    /// 결과에 우리 id가 없으면 빈 문자열(패닉 없음).
    #[test]
    fn trigger_description_missing_is_empty() {
        assert_eq!(trigger_description(&Results::new()), "");
    }
}
