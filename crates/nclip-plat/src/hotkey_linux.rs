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
    /// 눌림(`Activated` — 해제는 무시). 값 = 우리 쪽 단축키 id(`a1`·`a2`·`a3`).
    Activated(String),
}

pub(crate) const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
pub(crate) const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
/// 첫 단축키 id(설명·트리거 조회용 대표) — 09-04: 목록은 셸이 준다(`a1` = 퀵 팝업).
pub const SHORTCUT_ID: &str = "a1";

/// 마지막 등록 결과의 조합 설명(호스트 안내용).
static TRIGGER: Mutex<String> = Mutex::new(String::new());

/// 셸이 확정한 조합 설명(등록 전/실패 = 빈 문자열).
#[must_use]
pub fn bound_trigger() -> String {
    TRIGGER.lock().map(|g| g.clone()).unwrap_or_default()
}

/// 등록 세대 — 다시 등록할 때마다 오른다. 옛 세대의 대기 스레드는 자기 세대가 아니면 물러난다.
static GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// 지금 살아 있는 포털 세션(연결 · 세션 경로) — 다시 등록할 때 닫는다.
static CURRENT: Mutex<Option<(Connection, OwnedObjectPath)>> = Mutex::new(None);

/// 포털 등록 + 신호 대기 스레드 기동. 실패도 `Bound { ok: false }`로 **반드시 한 번** 알린다.
///
/// ★ 다시 불러도 된다(10-05 · T-38) — 설정에서 단축키를 바꾸면 옛 세션을 닫고 새 조합으로 새 세션을 연다.
/// 종전에는 기동 때 한 번만 등록해, 바꾼 단축키가 다음 시작에야 먹었다(Windows·mac은 즉시).
pub fn spawn(
    binds: Vec<(String, String, String)>,
    on_event: Box<dyn Fn(HotkeyEvent) + Send + Sync>,
) {
    use std::sync::atomic::Ordering;
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = std::thread::Builder::new()
        .name("nclip-hotkey".into())
        .spawn(move || match run(&binds, &on_event, gen) {
            Ok(()) => {}
            Err(e) => {
                // ★ 사유를 버리지 않는다(09-05) — 종전엔 `Err(_)`라 셸이 "포털이 없거나 거부됨"
                //   같은 **추측 목록**만 찍어 진단이 불가능했다. 포털 오류를 그대로 남긴다.
                eprintln!("전역 단축키: 포털 등록 실패 — {e}");
                on_event(HotkeyEvent::Bound {
                    ok: false,
                    trigger: String::new(),
                });
            }
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

/// `binds` = (id, 설명, preferred_trigger) 목록(09-04 — 동작별).
fn run(
    binds: &[(String, String, String)],
    on_event: &(dyn Fn(HotkeyEvent) + Send + Sync),
    gen: u32,
) -> zbus::Result<()> {
    use std::sync::atomic::Ordering;
    // 옛 세션 닫기 — 닫아야 셸이 옛 조합을 놓는다(같은 조합을 새 세션이 다시 잡을 수 있다).
    if let Some((old_conn, old_session)) = CURRENT.lock().ok().and_then(|mut g| g.take()) {
        let _ = old_conn.call_method(
            Some(PORTAL_DEST),
            old_session.as_str(),
            Some("org.freedesktop.portal.Session"),
            "Close",
            &(),
        );
    }
    // 요청·세션 토큰은 세대마다 달라야 한다(같은 경로를 다시 쓰면 응답이 섞인다).
    let (tok_session_req, tok_session, tok_bind) = (
        format!("nclip_req_session{gen}"),
        format!("nclip_session{gen}"),
        format!("nclip_req_bind{gen}"),
    );
    let conn = Connection::session()?;
    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE)?;
    // 포털 부재 = 여기서 실패(version 속성 조회).
    let _version: u32 = portal.get_property("version")?;

    // ① 세션.
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from(tok_session_req.as_str()));
    opts.insert("session_handle_token", Value::from(tok_session.as_str()));
    let (code, results) =
        call_with_response(&conn, &portal, "CreateSession", &(opts,), &tok_session_req)?;
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
    let mut shortcuts: Vec<(&str, HashMap<&str, Value<'_>>)> = Vec::new();
    for (id, desc, trig) in binds {
        let mut sc: HashMap<&str, Value<'_>> = HashMap::new();
        sc.insert("description", Value::from(desc.as_str()));
        sc.insert("preferred_trigger", Value::from(trig.as_str()));
        shortcuts.push((id.as_str(), sc));
    }
    let mut opts: HashMap<&str, Value<'_>> = HashMap::new();
    opts.insert("handle_token", Value::from(tok_bind.as_str()));
    let (code, results) = call_with_response(
        &conn,
        &portal,
        "BindShortcuts",
        &(&session, shortcuts, "", opts),
        &tok_bind,
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
    //   ★ 쥐고 있는 동안의 반복은 버린다(10-04 사용자 — "떼기 전에는 1회만") — [`RepeatFilter`].
    if let Ok(mut g) = CURRENT.lock() {
        *g = Some((conn.clone(), session.clone()));
    }
    let first_gap = key_repeat_delay_ms() + REPEAT_SLACK_MS;
    let mut filters: HashMap<String, RepeatFilter> = HashMap::new();
    let started = std::time::Instant::now();
    for msg in activated {
        // 다시 등록됐으면(세대가 바뀜) 이 스레드는 물러난다.
        if GEN.load(Ordering::SeqCst) != gen {
            return Ok(());
        }
        let Ok((s, id, ts, _o)) = msg
            .body()
            .deserialize::<(OwnedObjectPath, String, u64, Results)>()
        else {
            continue;
        };
        if s != session {
            continue; // 다른 세션의 신호.
        }
        // 포털 시각(ms)이 없으면(0) 받은 시각으로 대신한다.
        #[allow(clippy::cast_possible_truncation)]
        let ts = if ts == 0 {
            started.elapsed().as_millis() as u64
        } else {
            ts
        };
        let f = filters
            .entry(id.clone())
            .or_insert_with(|| RepeatFilter::new(first_gap));
        if f.admit(ts) {
            on_event(HotkeyEvent::Activated(id));
        }
    }
    Ok(())
}

/// 반복 판정 여유(ms) — 컴포지터 타이머·D-Bus 전달의 흔들림.
const REPEAT_SLACK_MS: u64 = 120;
/// 반복이 시작된 뒤의 간격 상한(ms) — GNOME 기본 반복 간격 30ms의 넉넉한 배수.
const REPEAT_CHAIN_MS: u64 = 200;

/// ★ 단축키 **쥐고 있는 동안의 반복**을 걸러낸다(10-04 사용자 — "계속 누르고 있어도 떼기 전에는 1회만" · T-57).
///
/// GNOME(mutter)은 전역 단축키를 쥐고 있으면 `Activated`를 **키 반복처럼 다시 보낸다** — 실측(mutter 50):
/// 첫 신호 뒤 500ms(키 반복 지연)에 둘째, 그 뒤 약 30ms 간격으로 계속 · `Deactivated`는 오지 않는다.
/// 그대로 받으면 팝업이 열렸다 닫혔다를 되풀이한다. Windows는 `MOD_NOREPEAT`, mac(Carbon)은 한 번만 준다.
///
/// 떼었다는 신호가 없으므로 **간격**으로 가른다(포털 시각 기준):
/// - 직전 신호 뒤 `first_gap`(반복 지연 + 여유) 안에 온 것 = 반복의 시작 → 버린다.
/// - 반복이 시작된 뒤에는 [`REPEAT_CHAIN_MS`] 안에 이어지는 한 계속 반복 → 버린다.
/// - 그보다 뜸을 두고 온 것 = 사용자가 뗐다가 **다시 누른 것** → 받는다(팝업 토글은 그대로 된다).
///
/// 알려진 한계: 팝업을 띄운 뒤 `first_gap` 안에 다시 누르는 아주 빠른 두 번 누름은 한 번으로 친다.
struct RepeatFilter {
    last: Option<u64>,
    repeating: bool,
    first_gap: u64,
}

impl RepeatFilter {
    fn new(first_gap: u64) -> Self {
        Self {
            last: None,
            repeating: false,
            first_gap,
        }
    }

    /// 이 신호를 **새 누름**으로 받아도 되는가. `ts` = 포털 시각(ms).
    fn admit(&mut self, ts: u64) -> bool {
        let Some(last) = self.last else {
            self.last = Some(ts);
            return true;
        };
        // 신호 순서가 살짝 뒤바뀌어 올 수 있다(실측) — 차이는 절댓값으로, 기준은 더 늦은 쪽으로.
        let gap = ts.abs_diff(last);
        self.last = Some(last.max(ts));
        let limit = if self.repeating {
            REPEAT_CHAIN_MS
        } else {
            self.first_gap
        };
        if gap <= limit {
            self.repeating = true;
            return false;
        }
        self.repeating = false;
        true
    }
}

/// GNOME 키 반복 지연(ms) — 못 읽으면 기본 500.
fn key_repeat_delay_ms() -> u64 {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.peripherals.keyboard", "delay"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .last()
                .and_then(|v| v.parse::<u64>().ok())
        })
        .filter(|v| (100..=2000).contains(v))
        .unwrap_or(500)
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
    /// ★ 쥐고 있는 동안의 반복은 버리고, 뗐다가 다시 누른 것은 받는다(10-04 실측 간격 — 500ms 뒤 30ms씩).
    #[test]
    fn held_shortcut_fires_once_and_repress_is_admitted() {
        let mut f = RepeatFilter::new(500 + REPEAT_SLACK_MS);
        let t0 = 169_603_696u64;
        assert!(f.admit(t0), "첫 누름");
        assert!(!f.admit(t0 + 500), "반복의 시작");
        let mut t = t0 + 500;
        for _ in 0..60 {
            t += 30;
            assert!(!f.admit(t), "쥐고 있는 동안의 반복");
        }
        // 순서가 뒤바뀐 신호도 반복이다.
        assert!(!f.admit(t - 30));
        // 뗐다가 0.4초 뒤 다시 누름 = 새 누름(토글).
        assert!(f.admit(t + 400), "다시 누름");
        // 그 누름도 쥐고 있으면 한 번만.
        assert!(!f.admit(t + 400 + 500));
        // 팝업을 띄우고 한참 뒤 다시 누름.
        let mut g = RepeatFilter::new(620);
        assert!(g.admit(1_000));
        assert!(g.admit(3_000), "뜸을 두고 다시 누름");
        // 짧게 눌렀다 뗀 뒤 0.7초 뒤 다시 누름 — 반복이 없었어도 받는다.
        assert!(g.admit(3_700));
    }
}
