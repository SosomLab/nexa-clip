//! 시스템 트레이 상주(T-12e · FR-U-2) — **플랫폼 어댑터**.
//!
//! 이식 원본: `nexa-beep` `crates/nbeep-plat/src/tray.rs`(1,102줄 · 3-OS).
//! 이식 = **공통 타입 + Windows 모듈**(08-28) + ★ **Linux SNI 모듈**(08-30 · 아래 `sni`)
//! + ★ **macOS NSStatusItem 모듈**(09-03 · 아래 `mac` — objc2는 winit과 같은 판 · 원장 docs/10 §3).
//!   미이식 타깃은 `spawn`이 `None`을 돌려 **정직하게 없다**고 알린다.
//!
//! ## Linux 구현 노트 (beep 08-15 설계 + 08-29 실기 그대로)
//!
//! - 트레이 = **SNI(StatusNotifierItem · D-Bus)** — 세션 버스에 `org.kde.StatusNotifierItem`과
//!   `com.canonical.dbusmenu`를 서빙하고 `StatusNotifierWatcher`에 등록. **워처가 없으면
//!   None**(GNOME은 AppIndicator 확장이 워처다 — 실측 `ubuntu-appindicators@ubuntu.com`).
//! - 메뉴 = dbusmenu(호스트 셸이 그린다 — OS 셸 영역은 OS 것). ★ 최근 항목(T-18e)은
//!   `100+i` id로 헤더 아래 깔린다.
//! - "열기"가 창을 **앞으로** 가져오려면 셸 발급 활성화 토큰이 필요하다(Wayland) —
//!   `ProvideXdgActivationToken`을 받아 두고 호스트가 [`take_activation_token`] +
//!   [`crate::wlactivate`]로 쓴다.
//! - 전역 단축키 = xdg 포털 `GlobalShortcuts`([`crate::hotkey_linux`]) — Windows의
//!   `RegisterHotKey`와 같은 자리(트레이 기동 시 등록 · 결과를 `HotkeyStatus`로).
//! - 알림 = `org.freedesktop.Notifications`(클릭 → 열기는 미배선 · 표시만).
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
    /// ★ 전역 단축키 눌림 — 값 = 동작 id([`nclip_core::hotkey`] ID_OPEN 1 · ID_OPEN_ALT 2 · ID_PASTE_PLAIN 3).
    Hotkey(u32),
    /// 전역 단축키 등록 결과(시작 직후 한 번) — 실패 = 다른 앱이 선점(충돌 표시용).
    HotkeyStatus(bool),
    /// 앱 종료(메뉴 "종료").
    Quit,
    /// ★ 설정 창 열기(메뉴 "설정" — 09-01 사용자 요청).
    Settings,
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
    /// ★ "설정" 라벨(i18n — 호스트 주입).
    pub settings_label: String,
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
pub use win::{cursor_pos, spawn};

#[cfg(target_os = "linux")]
pub use sni::{cursor_pos, spawn};

#[cfg(target_os = "macos")]
pub use mac::{cursor_pos, spawn};

/// Wayland 활성화 토큰(Linux SNI — `ProvideXdgActivationToken`으로 셸이 준 것) — 다음
/// Open 처리에서 **한 번** 꺼내 쓴다(토큰은 1회용). 다른 OS·미제공 = None.
#[must_use]
pub fn take_activation_token() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        sni::take_token()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// ★ 등록할 단축키 목록(09-04) — (동작 id, 조합). 셸이 설정에서 만들어 [`set_hotkeys`]로 넘긴다.
static HOTKEYS: std::sync::Mutex<Vec<(u32, nclip_core::hotkey::Hotkey)>> =
    std::sync::Mutex::new(Vec::new());
/// 사람이 읽는 조합 설명(호스트 안내용) — 셸이 함께 준다.
static HOTKEY_LABEL: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// ★ 단축키 목록 지정(09-04). 트레이 기동 전에 부르면 기동 때 등록되고, 기동 뒤에 부르면 **Windows·mac은 즉시 재등록**
/// (Windows = 트레이 스레드 메시지 · mac = 메인 스레드 Carbon 재등록 09-04) · Linux는 다음 시작에 반영된다(설명문에 명시).
pub fn set_hotkeys(list: Vec<(u32, nclip_core::hotkey::Hotkey)>, label: String) {
    if let Ok(mut g) = HOTKEYS.lock() {
        *g = list;
    }
    if let Ok(mut g) = HOTKEY_LABEL.lock() {
        *g = label;
    }
    #[cfg(windows)]
    win::rebind_hotkeys();
    #[cfg(target_os = "macos")]
    mac::rebind_hotkeys();
}

/// 지금 목록(플랫폼 등록 코드가 읽는다).
fn hotkeys() -> Vec<(u32, nclip_core::hotkey::Hotkey)> {
    HOTKEYS.lock().map(|g| g.clone()).unwrap_or_default()
}

/// 전역 단축키의 **실제** 조합 설명(호스트 안내용) — Linux는 셸이 확정한 것(사용자가
/// 포털 대화창에서 바꿨을 수 있다) · 그 외는 셸이 준 설명(없으면 기본).
#[must_use]
pub fn hotkey_label() -> String {
    #[cfg(target_os = "linux")]
    {
        let t = crate::hotkey_linux::bound_trigger();
        if !t.is_empty() {
            return t;
        }
    }
    let l = HOTKEY_LABEL.lock().map(|g| g.clone()).unwrap_or_default();
    if l.is_empty() {
        "Shift+Alt+C".to_string()
    } else {
        l
    }
}

/// 단축키 등록 실패 시 OS별 사실 안내(호스트가 그대로 보여준다).
#[must_use]
pub fn hotkey_failure_hint() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "xdg 포털 GlobalShortcuts가 없거나(GNOME 48+ · KDE 6+ 필요) 대화창에서 거부됨"
    }
    #[cfg(not(target_os = "linux"))]
    {
        "다른 앱(CopyQ 등)이 같은 조합을 쓰고 있음"
    }
}

/// 트레이를 못 띄웠을 때 OS별 사실 안내.
#[must_use]
pub fn tray_failure_hint() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "세션 버스에 StatusNotifierWatcher가 없음 — GNOME은 AppIndicator 확장(ubuntu-appindicators 등)을 켜야 한다"
    }
    #[cfg(windows)]
    {
        "트레이 창 생성 실패"
    }
    #[cfg(target_os = "macos")]
    {
        "AppKit 계약 위반(메인 스레드 밖 기동) — 재실행"
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        "이 OS는 아직 미이식"
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
/// 커서 위치(스텁) — 팝업 위치 계산용.
#[must_use]
pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
/// 스텁(미이식 타깃) — 트레이 없음. 호스트는 `None`을 보고 정직하게 알린다.
pub fn spawn<F: Fn(TrayEvent) + Send + Sync + 'static>(
    _content: TrayContent,
    _on_event: F,
) -> Option<TrayHandle> {
    None
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl TrayHandle {
    /// 표시 내용 갱신(스텁 — 도달 불가).
    pub fn update(&self, _content: TrayContent) {}
    /// 풍선 알림(스텁 — 도달 불가).
    pub fn notify(&self, _title: &str, _body: &str, _silent: bool, _target: &str) {}
}

/// Linux — **StatusNotifierItem(SNI · D-Bus)** 어댑터(이식 원본 beep `tray.rs::sni` +
/// 최근 항목·알림·포털 단축키 확장).
///
/// - 아이콘 = `IconPixmap`(ARGB32 **네트워크 바이트 순서** — SNI 규격) · 갱신 =
///   `NewIcon`/`NewToolTip`/`NewTitle` + dbusmenu `LayoutUpdated` 신호.
/// - 좌클릭 = `Activate`(열기) · 우클릭 = dbusmenu(셸이 그린다).
#[cfg(target_os = "linux")]
mod sni {
    use super::{TrayContent, TrayEvent, TrayHandle};
    use crate::hotkey_linux::{self, HotkeyEvent};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use zbus::blocking::Connection;
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    static STATE: OnceLock<Mutex<TrayContent>> = OnceLock::new();
    static ON_EVENT: OnceLock<Box<dyn Fn(TrayEvent) + Send + Sync>> = OnceLock::new();
    static CONN: OnceLock<Connection> = OnceLock::new();
    /// 셸이 넘긴 xdg_activation 토큰(1회용 · Open 직전에 도착).
    static TOKEN: Mutex<Option<String>> = Mutex::new(None);
    /// dbusmenu 레이아웃 리비전(갱신마다 증가 — 호스트가 재조회).
    static MENU_REV: AtomicU32 = AtomicU32::new(1);

    pub(super) fn take_token() -> Option<String> {
        TOKEN.lock().ok().and_then(|mut g| g.take())
    }

    /// SNI 픽스맵 — `(w, h, ARGB32)` 목록(규격 시그니처 `a(iiay)`).
    type Pixmaps = Vec<(i32, i32, Vec<u8>)>;
    /// SNI 툴팁 — `(아이콘명, 픽스맵, 제목, 본문)`.
    type ToolTip = (String, Pixmaps, String, String);
    /// dbusmenu 항목 — `(id, 속성, 자식)`.
    type MenuNode = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

    const ITEM_PATH: &str = "/StatusNotifierItem";
    const MENU_PATH: &str = "/MenuBar";
    /// 메뉴 항목 id — 1 헤더(비활성) · 2 구분선 · 3 열기 · 4 종료 · 5 구분선(최근 아래) ·
    /// **100+i = 최근 항목 i**(T-18e).
    const ID_HEADER: i32 = 1;
    const ID_SEP: i32 = 2;
    const ID_OPEN: i32 = 3;
    const ID_QUIT: i32 = 4;
    const ID_SEP2: i32 = 5;
    /// ★ 설정(09-01) — 열기와 종료 사이.
    const ID_SETTINGS: i32 = 6;
    const ID_RECENT_BASE: i32 = 100;

    fn emit(ev: TrayEvent) {
        if let Some(cb) = ON_EVENT.get() {
            cb(ev);
        }
    }

    fn state() -> TrayContent {
        STATE
            .get()
            .and_then(|m| m.lock().ok().map(|g| g.clone()))
            .unwrap_or_default()
    }

    /// RGBA(straight) → SNI `IconPixmap`(ARGB32 · 네트워크 바이트 순서 = [A,R,G,B]).
    fn argb_pixmap() -> Pixmaps {
        let c = state();
        let px = (c.side as usize) * (c.side as usize);
        if c.side == 0 || c.rgba.len() < px * 4 {
            return Vec::new();
        }
        let mut argb = Vec::with_capacity(px * 4);
        for p in c.rgba[..px * 4].chunks_exact(4) {
            argb.extend_from_slice(&[p[3], p[0], p[1], p[2]]);
        }
        let side = i32::try_from(c.side).unwrap_or(32);
        vec![(side, side, argb)]
    }

    /// `Value` → `OwnedValue`(실패 = None — fd 없는 값이라 실질 불가).
    fn ov<'a>(v: impl Into<Value<'a>>) -> Option<OwnedValue> {
        v.into().try_to_owned().ok()
    }

    /// 메뉴 항목 속성 집합.
    fn item_props(id: i32) -> HashMap<String, OwnedValue> {
        let c = state();
        let mut m = HashMap::new();
        let mut put = |k: &str, v: Option<OwnedValue>| {
            if let Some(v) = v {
                m.insert(k.to_string(), v);
            }
        };
        match id {
            ID_HEADER => {
                put("label", ov(c.name));
                put("enabled", ov(false));
            }
            ID_SEP | ID_SEP2 => put("type", ov("separator")),
            ID_OPEN => put("label", ov(c.open_label)),
            ID_SETTINGS => put("label", ov(c.settings_label)),
            ID_QUIT => put("label", ov(c.quit_label)),
            i if i >= ID_RECENT_BASE => {
                let idx = (i - ID_RECENT_BASE) as usize;
                if let Some(label) = c.recent.get(idx) {
                    // dbusmenu는 `_`를 니모닉으로 먹는다 — `__`로 이스케이프.
                    put("label", ov(label.replace('_', "__")));
                }
            }
            _ => {}
        }
        m
    }

    /// 루트의 자식 id 순서 — 헤더 · 구분선 · 최근… · (구분선) · 열기 · 종료.
    fn root_children() -> Vec<i32> {
        let n = state().recent.len();
        let mut ids = vec![ID_HEADER, ID_SEP];
        for i in 0..n {
            ids.push(ID_RECENT_BASE + i32::try_from(i).unwrap_or(0));
        }
        if n > 0 {
            ids.push(ID_SEP2);
        }
        ids.push(ID_OPEN);
        ids.push(ID_SETTINGS);
        ids.push(ID_QUIT);
        ids
    }

    /// (id, 속성, 자식 없음) 구조 — dbusmenu 항목 `(ia{sv}av)`.
    fn item_value(id: i32) -> Option<OwnedValue> {
        ov((id, item_props(id), Vec::<OwnedValue>::new()))
    }

    fn on_click(id: i32) {
        match id {
            ID_OPEN => emit(TrayEvent::Open),
            ID_SETTINGS => emit(TrayEvent::Settings),
            ID_QUIT => emit(TrayEvent::Quit),
            i if i >= ID_RECENT_BASE => emit(TrayEvent::Recent((i - ID_RECENT_BASE) as usize)),
            _ => {}
        }
    }

    /// `org.kde.StatusNotifierItem` — 호스트(트레이 영역)가 읽는다.
    struct Item;

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl Item {
        #[zbus(property)]
        fn category(&self) -> String {
            "ApplicationStatus".into()
        }
        #[zbus(property)]
        fn id(&self) -> String {
            "nexa-clip".into()
        }
        #[zbus(property)]
        fn title(&self) -> String {
            state().name
        }
        #[zbus(property)]
        fn status(&self) -> String {
            "Active".into()
        }
        #[zbus(property)]
        fn icon_name(&self) -> String {
            String::new() // 픽스맵만 쓴다(코드로 그린 아이콘 — 테마 아이콘 없음)
        }
        #[zbus(property)]
        fn icon_pixmap(&self) -> Pixmaps {
            argb_pixmap()
        }
        #[zbus(property)]
        fn tool_tip(&self) -> ToolTip {
            (String::new(), Vec::new(), state().tooltip, String::new())
        }
        #[zbus(property)]
        fn menu(&self) -> ObjectPath<'static> {
            ObjectPath::from_static_str_unchecked(MENU_PATH)
        }
        #[zbus(property)]
        fn item_is_menu(&self) -> bool {
            false // 좌클릭 = Activate(열기) · 메뉴는 우클릭(호스트 관례)
        }
        fn activate(&self, _x: i32, _y: i32) {
            emit(TrayEvent::Open);
        }
        /// GNOME appindicator 확장이 클릭 직전에 넘기는 정식 활성화 토큰(beep 08-29) —
        /// 좌클릭·메뉴 항목 모두 선행 호출. 저장만 하고 Open 처리에서 소비한다.
        fn provide_xdg_activation_token(&self, token: String) {
            if let Ok(mut g) = TOKEN.lock() {
                *g = Some(token);
            }
        }
        fn secondary_activate(&self, _x: i32, _y: i32) {
            emit(TrayEvent::Open);
        }
        fn context_menu(&self, _x: i32, _y: i32) {
            // 메뉴 렌더는 dbusmenu를 읽는 호스트 몫 — 여기 올 일은 드물다(no-op).
        }
        fn scroll(&self, _delta: i32, _orientation: String) {}
    }

    /// `com.canonical.dbusmenu` — 최소 구현(깊이 1 · 클릭 이벤트만).
    struct Menu;

    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl Menu {
        #[zbus(property)]
        fn version(&self) -> u32 {
            3
        }
        #[zbus(property)]
        fn status(&self) -> String {
            "normal".into()
        }
        #[zbus(property)]
        fn text_direction(&self) -> String {
            "ltr".into()
        }
        #[zbus(property)]
        fn icon_theme_path(&self) -> Vec<String> {
            Vec::new()
        }

        /// 레이아웃 — 루트(0) 요청에만 자식을 준다(깊이 1 고정 메뉴).
        fn get_layout(
            &self,
            parent_id: i32,
            _recursion_depth: i32,
            _property_names: Vec<String>,
        ) -> zbus::fdo::Result<(u32, MenuNode)> {
            let rev = MENU_REV.load(Ordering::Relaxed);
            let children = if parent_id == 0 {
                root_children().into_iter().filter_map(item_value).collect()
            } else {
                Vec::new()
            };
            Ok((rev, (0, HashMap::new(), children)))
        }

        fn get_group_properties(
            &self,
            ids: Vec<i32>,
            _property_names: Vec<String>,
        ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
            ids.into_iter().map(|id| (id, item_props(id))).collect()
        }

        fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
            item_props(id).remove(&name).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(format!("항목 {id}에 속성 {name} 없음"))
            })
        }

        /// 클릭 처리 — 열기/종료/최근 항목.
        fn event(&self, id: i32, event_id: String, _data: Value<'_>, _timestamp: u32) {
            if event_id == "clicked" {
                on_click(id);
            }
        }

        fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
            for (id, event_id, _d, _t) in events {
                if event_id == "clicked" {
                    on_click(id);
                }
            }
            Vec::new()
        }

        fn about_to_show(&self, _id: i32) -> bool {
            false
        }

        fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
            (Vec::new(), Vec::new())
        }
    }

    /// 커서 위치 — X11 세션이면 `QueryPointer`, Wayland는 **없다**(클라이언트가 전역 좌표를
    /// 못 본다 — 팝업은 컴포지터가 놓는 자리에 뜬다).
    #[must_use]
    pub fn cursor_pos() -> Option<(i32, i32)> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return None;
        }
        use x11rb::connection::Connection as _;
        use x11rb::protocol::xproto::ConnectionExt as _;
        let (conn, screen) = x11rb::rust_connection::RustConnection::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let p = conn.query_pointer(root).ok()?.reply().ok()?;
        Some((i32::from(p.root_x), i32::from(p.root_y)))
    }

    /// 세션 버스에 서빙 + 워처 등록 + 포털 단축키 등록. 실패(버스 부재·워처 부재)는 None.
    pub fn spawn<F: Fn(TrayEvent) + Send + Sync + 'static>(
        content: TrayContent,
        on_event: F,
    ) -> Option<TrayHandle> {
        if STATE.set(Mutex::new(content)).is_err() {
            return None; // 이미 떠 있다
        }
        let _ = ON_EVENT.set(Box::new(on_event));
        let conn = Connection::session().ok()?;
        conn.object_server().at(ITEM_PATH, Item).ok()?;
        conn.object_server().at(MENU_PATH, Menu).ok()?;
        let unique = conn.unique_name()?.to_string();
        // 워처 등록 — 이게 없으면 트레이를 그릴 호스트가 없다(fail-soft: None).
        conn.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &unique,
        )
        .ok()?;
        let _ = CONN.set(conn);
        // ★ 전역 단축키(T-15 Linux) — Windows `RegisterHotKey`와 같은 자리. 결과는 한 번 알린다.
        // ★ 목록(09-04) — 동작 id별 포털 단축키(설명은 셸 대화창에 보인다). 런타임 변경은 다음 시작에.
        let name = state().name;
        let binds: Vec<(String, String, String)> = super::hotkeys()
            .into_iter()
            .map(|(id, hk)| {
                let what = match id {
                    nclip_core::hotkey::ID_PASTE_PLAIN => "평문 붙여넣기",
                    nclip_core::hotkey::ID_OPEN_ALT => "퀵 팝업(보조)",
                    _ => "퀵 팝업",
                };
                (
                    format!("a{id}"),
                    format!("{name} — {what}"),
                    hk.portal_spec(),
                )
            })
            .collect();
        hotkey_linux::spawn(
            binds,
            Box::new(|ev| match ev {
                HotkeyEvent::Bound { ok, .. } => emit(TrayEvent::HotkeyStatus(ok)),
                HotkeyEvent::Activated(id) => {
                    let n = id
                        .strip_prefix('a')
                        .and_then(|x| x.parse::<u32>().ok())
                        .unwrap_or(1);
                    emit(TrayEvent::Hotkey(n));
                }
            }),
        );
        Some(TrayHandle { _priv: () })
    }

    impl TrayHandle {
        /// 표시 내용 갱신 — 신호(NewIcon 등)로 호스트가 재조회한다.
        pub fn update(&self, content: TrayContent) {
            if let Some(s) = STATE.get() {
                if let Ok(mut g) = s.lock() {
                    *g = content;
                }
            }
            let rev = MENU_REV.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(conn) = CONN.get() {
                let iface = "org.kde.StatusNotifierItem";
                for sig in ["NewIcon", "NewToolTip", "NewTitle"] {
                    let _ = conn.emit_signal(
                        None::<zbus::names::BusName<'_>>,
                        ITEM_PATH,
                        iface,
                        sig,
                        &(),
                    );
                }
                let _ = conn.emit_signal(
                    None::<zbus::names::BusName<'_>>,
                    MENU_PATH,
                    "com.canonical.dbusmenu",
                    "LayoutUpdated",
                    &(rev, 0i32),
                );
            }
        }

        /// 알림 — `org.freedesktop.Notifications.Notify`(데스크톱 표준). `target`은 미배선
        /// (클릭 → 열기는 `ActionInvoked` 구독이 필요 — 후속) · `silent` = 소리 억제 힌트.
        pub fn notify(&self, title: &str, body: &str, silent: bool, _target: &str) {
            let Some(conn) = CONN.get() else { return };
            let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
            hints.insert("suppress-sound", Value::from(silent));
            let _ = conn.call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    state().name,
                    0u32,
                    "",
                    title,
                    body,
                    Vec::<&str>::new(),
                    hints,
                    -1i32,
                ),
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 최근 항목이 없으면 구분선 5가 없고, 있으면 헤더 아래에 100+i가 깔린다.
        #[test]
        fn root_children_layout_follows_recent() {
            let _ = STATE.set(Mutex::new(TrayContent::default()));
            let ids = root_children();
            assert_eq!(ids, vec![ID_HEADER, ID_SEP, ID_OPEN, ID_SETTINGS, ID_QUIT]);
            if let Some(s) = STATE.get() {
                if let Ok(mut g) = s.lock() {
                    g.recent = vec!["a".into(), "b_c".into()];
                }
            }
            let ids = root_children();
            assert_eq!(
                ids,
                vec![
                    ID_HEADER,
                    ID_SEP,
                    100,
                    101,
                    ID_SEP2,
                    ID_OPEN,
                    ID_SETTINGS,
                    ID_QUIT
                ]
            );
            // 니모닉 이스케이프.
            let label = item_props(101)
                .remove("label")
                .map(|v| String::try_from(v).ok());
            assert_eq!(label.flatten().as_deref(), Some("b__c"));
        }
    }
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
        fn RegisterHotKey(hwnd: HWND, id: i32, modifiers: u32, vk: u32) -> i32;
        fn UnregisterHotKey(hwnd: HWND, id: i32) -> i32;
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
    const WM_HOTKEY: u32 = 0x0312;
    /// ★ 전역 단축키(09-04 설정 배선) — 목록은 [`super::hotkeys`] · id = 동작 id(1..=8) · 재등록은 `WM_APP_HOTKEY`.
    const HOTKEY_ID_MAX: i32 = 8;
    /// 재등록 방지(Windows 10 1607+) — 키 반복으로 이벤트가 쏟아지지 않게.
    const MOD_NOREPEAT: u32 = 0x4000;
    const WM_APP_HOTKEY: u32 = 0x8000 + 4; // 단축키 목록 재등록 요청(목록은 HOTKEYS에)
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
    /// ★ 설정(09-01).
    const CMD_SETTINGS: usize = 3;
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

    /// 현재 커서의 화면 좌표 — 팝업을 커서 위치에 띄운다(DR-24 `ui.popup_at` 기본).
    #[must_use]
    pub fn cursor_pos() -> Option<(i32, i32)> {
        let mut pt = Point { x: 0, y: 0 };
        // SAFETY: 출력 구조체 포인터만 넘기는 조회 호출.
        (unsafe { GetCursorPos(&mut pt) } != 0).then_some((pt.x, pt.y))
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn emit(ev: TrayEvent) {
        if let Some(cb) = ON_EVENT.get() {
            cb(ev);
        }
    }

    /// ★ 목록대로 (재)등록(09-04) — 트레이 스레드에서만. 옛 id는 전부 해제한 뒤 등록. 반환 = 하나라도 있고 전부 성공.
    fn register_hotkeys(hwnd: HWND) -> bool {
        // SAFETY: 이 스레드가 만든 hwnd · Win32 등록/해제 호출.
        unsafe {
            for id in 1..=HOTKEY_ID_MAX {
                UnregisterHotKey(hwnd, id);
            }
            let list = super::hotkeys();
            let mut all = !list.is_empty();
            for (id, hk) in list {
                if !(1..=HOTKEY_ID_MAX as u32).contains(&id) {
                    continue;
                }
                let ok = RegisterHotKey(
                    hwnd,
                    id as i32,
                    hk.win_mods() | MOD_NOREPEAT,
                    hk.key.win_vk(),
                ) != 0;
                if !ok {
                    all = false;
                }
            }
            all
        }
    }

    /// 셸이 목록을 바꿨다 — 트레이 스레드에 재등록을 시킨다(창이 아직 없으면 기동 때 등록된다).
    pub(super) fn rebind_hotkeys() {
        let hwnd = HWND.load(Ordering::Acquire);
        if hwnd != 0 {
            // SAFETY: 살아 있는 hwnd에 사용자 메시지 게시.
            unsafe {
                PostMessageW(hwnd, WM_APP_HOTKEY, 0, 0);
            }
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
        let (name, open_label, settings_label, quit_label, recent) = match state.lock() {
            Ok(g) => (
                g.name.clone(),
                g.open_label.clone(),
                g.settings_label.clone(),
                g.quit_label.clone(),
                g.recent.clone(),
            ),
            Err(_) => return,
        };
        let name_w = wide(&name);
        let open_w = wide(&open_label);
        let settings_w = wide(&settings_label);
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
            AppendMenuW(menu, MF_STRING, CMD_SETTINGS, settings_w.as_ptr());
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
                CMD_SETTINGS => emit(TrayEvent::Settings),
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
            WM_HOTKEY => {
                if (1..=HOTKEY_ID_MAX as usize).contains(&w) {
                    emit(TrayEvent::Hotkey(w as u32));
                }
                0
            }
            WM_APP_HOTKEY => {
                let ok = register_hotkeys(hwnd);
                emit(TrayEvent::HotkeyStatus(ok));
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
                    // ★ 전역 단축키(T-15 · 09-04 설정 배선) — 이 스레드의 메시지 루프가 WM_HOTKEY를 받는다.
                    //   실패 = 다른 앱이 선점(CopyQ 등) — 조용히 넘기지 않고 알린다.
                    let hot = register_hotkeys(hwnd);
                    emit(TrayEvent::HotkeyStatus(hot));
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

/// macOS — **메뉴바 NSStatusItem** 어댑터(T-12e mac · 09-03 사용자 "윈도우와 동일하게
/// 메뉴바 상주"). 이식 원본: `nexa-beep` `crates/nbeep-plat/src/tray.rs::mac`(M3-2b)
/// + **최근 항목·설정 메뉴 확장**(Windows/Linux 모듈과 동일 메뉴 구성).
///
/// - **AppKit은 메인 스레드 강제** — `spawn`/`update`는 winit 메인 루프에서만 불린다
///   (셸 기동·refresh_tray가 그 자리). 메인 스레드가 아니면 `None`(fail-soft).
/// - 아이콘 = 호스트 합성 RGBA → `NSBitmapImageRep` → `NSImage`(표시 18×18pt —
///   32px 원본이라 레티나에서 2x로 선명). ★ 연결 배지(녹색 점)는 호스트가 RGBA에
///   합성해 넘긴다 — `update` 한 번이 곧 "연결 시 아이콘 변경"이다.
/// - 메뉴 = **좌/우클릭 공통**(mac 관례 — beep 분석 08-15): 이름 헤더(비활성) · 구분선 ·
///   최근 항목(태그 = 인덱스) · 구분선 · 열기 · 설정 · 구분선 · 종료. 최근 항목 수가
///   매번 달라 `update`마다 메뉴를 통째로 다시 깐다(항목 수십 개 — 비용 무시 가능).
/// - 상태(NSStatusItem — `Send` 아님)는 **스레드 로컬**. `TrayHandle`은 표식일 뿐이고,
///   메인 스레드 밖 `update`는 조용히 무시된다(도달 경로 없음).
/// - 전역 단축키·정식 알림은 후속(T-15 mac · UNUserNotificationCenter는 번들 필요).
#[cfg(target_os = "macos")]
mod mac {
    use super::{TrayContent, TrayEvent, TrayHandle};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
    use objc2_app_kit::{
        NSBitmapImageRep, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
        NSVariableStatusItemLength,
    };
    use objc2_foundation::{MainThreadMarker, NSSize, NSString};
    use std::cell::RefCell;
    use std::sync::OnceLock;

    /// 이벤트 콜백(호스트 프록시 래퍼) — 액션 셀렉터에서 부른다.
    static ON_EVENT: OnceLock<Box<dyn Fn(TrayEvent) + Send + Sync>> = OnceLock::new();

    fn emit(ev: TrayEvent) {
        if let Some(f) = ON_EVENT.get() {
            f(ev);
        }
    }

    struct State {
        item: Retained<NSStatusItem>,
        menu: Retained<NSMenu>,
        target: Retained<Target>,
    }

    thread_local! {
        /// 메인 스레드 전용 상태(NSStatusItem은 Send가 아니다).
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    declare_class!(
        /// 메뉴 액션 수신자 — 셀렉터를 콜백으로 잇는 것 외에 아무것도 모른다.
        struct Target;

        unsafe impl ClassType for Target {
            type Super = NSObject;
            type Mutability = mutability::InteriorMutable;
            const NAME: &'static str = "NclipTrayTarget";
        }

        impl DeclaredClass for Target {
            type Ivars = ();
        }

        unsafe impl Target {
            #[method(nclipTrayOpen:)]
            fn tray_open(&self, _sender: Option<&AnyObject>) {
                emit(TrayEvent::Open);
            }

            #[method(nclipTraySettings:)]
            fn tray_settings(&self, _sender: Option<&AnyObject>) {
                emit(TrayEvent::Settings);
            }

            #[method(nclipTrayQuit:)]
            fn tray_quit(&self, _sender: Option<&AnyObject>) {
                emit(TrayEvent::Quit);
            }

            #[method(nclipTrayRecent:)]
            fn tray_recent(&self, sender: Option<&AnyObject>) {
                let Some(s) = sender else { return };
                // ★ 최근 항목 인덱스는 메뉴 항목 tag에 실려 온다(0 = 최신).
                let tag: isize = unsafe { msg_send![s, tag] };
                if let Ok(i) = usize::try_from(tag) {
                    emit(TrayEvent::Recent(i));
                }
            }
        }
    );

    /// RGBA(straight) → NSImage(18×18pt 표시 · 원본 해상도 유지 = 레티나 2x).
    fn image_from_rgba(rgba: &[u8], side: u32) -> Option<Retained<NSImage>> {
        if side == 0 || rgba.len() != (side as usize) * (side as usize) * 4 {
            return None;
        }
        unsafe {
            let rep: Option<Retained<NSBitmapImageRep>> = msg_send_id![
                NSBitmapImageRep::alloc(),
                initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
                pixelsWide: side as isize,
                pixelsHigh: side as isize,
                bitsPerSample: 8_isize,
                samplesPerPixel: 4_isize,
                hasAlpha: true,
                isPlanar: false,
                colorSpaceName: &*NSString::from_str("NSDeviceRGBColorSpace"),
                bytesPerRow: (side * 4) as isize,
                bitsPerPixel: 32_isize,
            ];
            let rep = rep?;
            let data = rep.bitmapData();
            if data.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), data, rgba.len());
            let img = NSImage::initWithSize(NSImage::alloc(), NSSize::new(18.0, 18.0));
            img.addRepresentation(&rep);
            Some(img)
        }
    }

    /// 메뉴 항목 하나(제목 + 액션 + 태그) — 반복을 줄인다.
    fn menu_item(
        mtm: MainThreadMarker,
        target: &Retained<Target>,
        title: &str,
        action: objc2::runtime::Sel,
        tag: isize,
    ) -> Retained<NSMenuItem> {
        let it = NSMenuItem::new(mtm);
        unsafe {
            it.setTitle(&NSString::from_str(title));
            it.setAction(Some(action));
            it.setTarget(Some(target));
            let _: () = msg_send![&*it, setTag: tag];
        }
        it
    }

    /// 아이콘·툴팁 반영 + 메뉴 재구성(Windows 모듈과 같은 구성 · T-18e 최근 항목 포함).
    fn apply(mtm: MainThreadMarker, state: &State, content: &TrayContent) {
        unsafe {
            if let Some(btn) = state.item.button(mtm) {
                if let Some(img) = image_from_rgba(&content.rgba, content.side) {
                    btn.setImage(Some(&img));
                }
                btn.setToolTip(Some(&NSString::from_str(&content.tooltip)));
            }
            let menu = &state.menu;
            menu.removeAllItems();
            let header = NSMenuItem::new(mtm);
            header.setTitle(&NSString::from_str(&content.name));
            // 액션 없는 항목은 autoenablesItems가 비활성으로 그린다(이름 헤더 관례).
            menu.addItem(&header);
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            for (i, label) in content.recent.iter().enumerate() {
                let Ok(tag) = isize::try_from(i) else { break };
                menu.addItem(&menu_item(
                    mtm,
                    &state.target,
                    label,
                    sel!(nclipTrayRecent:),
                    tag,
                ));
            }
            if !content.recent.is_empty() {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            menu.addItem(&menu_item(
                mtm,
                &state.target,
                &content.open_label,
                sel!(nclipTrayOpen:),
                0,
            ));
            menu.addItem(&menu_item(
                mtm,
                &state.target,
                &content.settings_label,
                sel!(nclipTraySettings:),
                0,
            ));
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            menu.addItem(&menu_item(
                mtm,
                &state.target,
                &content.quit_label,
                sel!(nclipTrayQuit:),
                0,
            ));
        }
    }

    /// 커서 위치(물리 px · 팝업 위치 계산용 — 09-04 mac 실기 "팝업이 다른 모니터로 간다"):
    /// CGEvent의 전역 좌표(포인트 · 주 화면 좌상단 원점)를 **그 포인트가 든 디스플레이의
    /// 배율**로 물리 px로 환산한다 — winit의 mac 모니터 좌표(`position`/`size`) 규약과 같다.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn cursor_pos() -> Option<(i32, i32)> {
        use std::ffi::c_void;
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CGPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        struct CGSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        struct CGRect {
            origin: CGPoint,
            size: CGSize,
        }
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreate(src: *mut c_void) -> *mut c_void;
            fn CGEventGetLocation(ev: *mut c_void) -> CGPoint;
            fn CGGetDisplaysWithPoint(p: CGPoint, max: u32, out: *mut u32, count: *mut u32) -> i32;
            fn CGDisplayBounds(display: u32) -> CGRect;
            fn CGDisplayPixelsWide(display: u32) -> usize;
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFRelease(cf: *mut c_void);
        }
        // SAFETY: 빈 이벤트를 만들어 현재 커서 위치만 읽고 즉시 해제 · 출력 포인터 조회 호출.
        unsafe {
            let ev = CGEventCreate(std::ptr::null_mut());
            if ev.is_null() {
                return None;
            }
            let p = CGEventGetLocation(ev);
            CFRelease(ev);
            let (mut display, mut n) = (0u32, 0u32);
            let scale = if CGGetDisplaysWithPoint(p, 1, &mut display, &mut n) == 0 && n > 0 {
                let b = CGDisplayBounds(display);
                if b.size.width > 0.0 {
                    CGDisplayPixelsWide(display) as f64 / b.size.width
                } else {
                    1.0
                }
            } else {
                1.0
            };
            Some(((p.x * scale).round() as i32, (p.y * scale).round() as i32))
        }
    }

    /// 메뉴바 상주 시작 — **메인 스레드에서만**(아니면 None · fail-soft).
    pub fn spawn<F: Fn(TrayEvent) + Send + Sync + 'static>(
        content: TrayContent,
        on_event: F,
    ) -> Option<TrayHandle> {
        let mtm = MainThreadMarker::new()?; // AppKit 계약 — 메인 스레드 증명
        if ON_EVENT.set(Box::new(on_event)).is_err() {
            return None; // 이미 떠 있다
        }
        let target: Retained<Target> = unsafe { msg_send_id![Target::alloc(), init] };
        unsafe {
            let bar = NSStatusBar::systemStatusBar();
            let item = bar.statusItemWithLength(NSVariableStatusItemLength);
            let menu = NSMenu::new(mtm);
            // mac 관례 — 메뉴를 달면 좌/우클릭 둘 다 연다(분석 표 08-15). "열기"는 메뉴에서.
            item.setMenu(Some(&menu));
            let state = State { item, menu, target };
            apply(mtm, &state, &content);
            STATE.with(|s| *s.borrow_mut() = Some(state));
        }
        // ★ 전역 단축키(T-15 mac · 09-04 사용자 "Ctrl+Shift+V가 안 됨") — Carbon
        //   `RegisterEventHotKey`(시스템 전역 · 손쉬운 사용 권한 불요). Windows
        //   `RegisterHotKey`와 같은 자리 — 결과를 한 번 알린다.
        emit(TrayEvent::HotkeyStatus(hotkey::register()));
        Some(TrayHandle { _priv: () })
    }

    impl TrayHandle {
        /// 표시 내용 갱신 — 메인 스레드에서만 실제 반영(밖이면 무시 · 도달 경로 없음).
        pub fn update(&self, content: TrayContent) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            STATE.with(|s| {
                if let Some(state) = s.borrow().as_ref() {
                    apply(mtm, state, &content);
                }
            });
        }

        /// 알림 — 미배선(정식 = UNUserNotificationCenter · .app 번들 필요 — 후속). 표식만.
        pub fn notify(&self, _title: &str, _body: &str, _silent: bool, _target: &str) {}
    }

    /// ★ Carbon 전역 단축키(T-15 mac) — `Ctrl+Shift+V` = 퀵 팝업(Windows·Linux와 동일 계약).
    ///
    /// Carbon Hot Key API는 **보조 접근 권한 없이** 시스템 전역으로 동작하는 유일한
    /// 공개 경로다(NSEvent 전역 모니터는 손쉬운 사용 권한 필요 · CGEventTap도 동일).
    /// deprecated이지만 arm64 포함 현행 macOS에서 지원되며 대체 API가 없다
    /// (Rust 생태 `global-hotkey` crate가 같은 선언을 쓴다 — 선언만 차용 · 의존 추가 없음).
    /// 이벤트는 애플리케이션 이벤트 타깃으로 오므로 winit(NSApplication) 루프가 배달한다.
    mod hotkey {
        use super::{emit, TrayEvent};
        use std::ffi::c_void;

        type OSStatus = i32;

        #[repr(C)]
        struct EventTypeSpec {
            class: u32,
            kind: u32,
        }

        #[repr(C)]
        struct EventHotKeyID {
            signature: u32,
            id: u32,
        }

        #[link(name = "Carbon", kind = "framework")]
        extern "C" {
            fn GetApplicationEventTarget() -> *mut c_void;
            fn InstallEventHandler(
                target: *mut c_void,
                handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> OSStatus,
                num_types: u32,
                list: *const EventTypeSpec,
                user_data: *mut c_void,
                out: *mut *mut c_void,
            ) -> OSStatus;
            fn RegisterEventHotKey(
                key_code: u32,
                modifiers: u32,
                id: EventHotKeyID,
                target: *mut c_void,
                options: u32,
                out: *mut *mut c_void,
            ) -> OSStatus;
            fn UnregisterEventHotKey(hotkey: *mut c_void) -> OSStatus;
        }

        thread_local! {
            /// 핸들러 설치 여부(1회) — 재등록 때 핸들러를 또 깔면 이벤트가 중복 배달된다.
            static HANDLER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
            /// 지금 등록된 단축키 핸들 — 재등록 때 전부 해제하고 새로 건다(09-04 즉시 반영).
            static REGS: std::cell::RefCell<Vec<*mut c_void>> = const { std::cell::RefCell::new(Vec::new()) };
        }

        extern "C" {
            fn GetEventParameter(
                event: *mut c_void,
                name: u32,
                typ: u32,
                out_type: *mut u32,
                size: u32,
                out_size: *mut u32,
                data: *mut c_void,
            ) -> OSStatus;
        }

        extern "C" fn on_hotkey(_: *mut c_void, event: *mut c_void, _: *mut c_void) -> OSStatus {
            // ★ 어느 단축키인지(09-04) — kEventParamDirectObject('----') · typeEventHotKeyID('hkid').
            let mut id = EventHotKeyID {
                signature: 0,
                id: 0,
            };
            // SAFETY: Carbon이 준 이벤트에서 고정 크기 구조체를 읽는다.
            let got = unsafe {
                GetEventParameter(
                    event,
                    u32::from_be_bytes(*b"----"),
                    u32::from_be_bytes(*b"hkid"),
                    std::ptr::null_mut(),
                    std::mem::size_of::<EventHotKeyID>() as u32,
                    std::ptr::null_mut(),
                    (&mut id as *mut EventHotKeyID).cast(),
                )
            } == 0;
            emit(TrayEvent::Hotkey(if got && id.id > 0 { id.id } else { 1 }));
            0 // noErr
        }

        /// ★ 목록([`super::super::hotkeys`])대로 등록(09-04) — 성공 여부(실패 = 다른 앱 선점 등 · 호스트가 알린다).
        ///   ★ **재호출 = 재등록**(09-04 사용자 실기 "단축키 설정이 안 바뀜"): 핸들러는 1회만 깔고,
        ///   기존 핸들을 전부 `UnregisterEventHotKey`한 뒤 새 목록을 건다 — Windows와 같은 즉시 반영.
        ///   메인 스레드 전용(호출부가 보장).
        pub(super) fn register() -> bool {
            const KEYBOARD: u32 = u32::from_be_bytes(*b"keyb"); // kEventClassKeyboard
            const HOTKEY_PRESSED: u32 = 5; // kEventHotKeyPressed
            let spec = EventTypeSpec {
                class: KEYBOARD,
                kind: HOTKEY_PRESSED,
            };
            // SAFETY: 출력 포인터만 받는 등록/해제 호출 — 핸들러는 앱 수명 전체 유지(1회).
            unsafe {
                let target = GetApplicationEventTarget();
                if !HANDLER.get() {
                    let mut handler: *mut c_void = std::ptr::null_mut();
                    if InstallEventHandler(
                        target,
                        on_hotkey,
                        1,
                        &spec,
                        std::ptr::null_mut(),
                        &mut handler,
                    ) != 0
                    {
                        return false;
                    }
                    HANDLER.set(true);
                }
                // 이전 등록 전부 해제 — 옛 조합이 남아 두 조합이 다 살아 있으면 안 된다.
                REGS.with(|r| {
                    for h in r.borrow_mut().drain(..) {
                        let _ = UnregisterEventHotKey(h);
                    }
                });
                let list = super::super::hotkeys();
                let mut all = !list.is_empty();
                for (n, hk) in list {
                    let id = EventHotKeyID {
                        signature: u32::from_be_bytes(*b"nclp"),
                        id: n,
                    };
                    let code = hk.key.mac_keycode();
                    if code == 0xFFFF {
                        all = false;
                        continue;
                    }
                    let mut hotkey: *mut c_void = std::ptr::null_mut();
                    if RegisterEventHotKey(code, hk.mac_mods(), id, target, 0, &mut hotkey) != 0 {
                        all = false;
                    } else {
                        REGS.with(|r| r.borrow_mut().push(hotkey));
                    }
                }
                all
            }
        }
    }

    /// ★ 런타임 재등록(09-04) — `set_hotkeys`가 부른다. 트레이가 떠 있고 메인 스레드일 때만
    ///   (기동 전 호출은 `spawn`이 등록하고, 다른 스레드면 다음 시작에 반영). 결과를 통지한다.
    pub(super) fn rebind_hotkeys() {
        if MainThreadMarker::new().is_none() {
            return;
        }
        let spawned = STATE.with(|s| s.borrow().is_some());
        if spawned {
            emit(TrayEvent::HotkeyStatus(hotkey::register()));
        }
    }
}
