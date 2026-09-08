//! ★ 전역 단축키 모델(09-04 사용자 — "단축키 지정 화면 · 캡처 창 · 여러 기능에 지정").
//!
//! 설정에는 **공통 문자열**로 저장한다(`Ctrl+Shift+Alt+Win+C` 순서 · 대소문자 무관 파싱). 표시는 OS 관례(mac은 ⌃⇧⌥⌘).
//! OS 등록에 필요한 숫자(Windows VK · mac Carbon 키코드 · 포털 spec)는 여기서 낸다 — 플랫폼 코드는 값만 받는다.
//!
//! 동작 id(플랫폼 이벤트가 되돌려 주는 번호): 1 = 퀵 팝업 · 2 = 퀵 팝업(보조) · 3 = 평문 붙여넣기.
//!
//! ★ **창 안 단축키**(09-08 사용자 요청 "각 단축키는 설정 가능하게 · 내장 단축키도 전부 항목에") —
//! [`WINDOW_ACTIONS`]. 같은 문자열 규약으로 저장하되 OS에 등록하지 않는다(팝업·메인창이 포커스일 때
//! 창이 직접 비교 — [`Hotkey::matches`]). 전역과 달리 `Enter`·`Delete` 같은 **맨 키**를 허용하고,
//! 글자·숫자 키만 수식 키를 요구한다([`Hotkey::is_window_safe`] — 검색창 입력과 겹치지 않게).

/// 단축키 동작 id — 설정 키와 1:1.
pub const ID_OPEN: u32 = 1;
pub const ID_OPEN_ALT: u32 = 2;
pub const ID_PASTE_PLAIN: u32 = 3;

/// (설정 키, 동작 id, 기본 조합) — 화면 순서.
pub const ACTIONS: &[(&str, u32, &str)] = &[
    ("key.open", ID_OPEN, "Shift+Alt+C"),
    ("key.open_alt", ID_OPEN_ALT, ""), // 보조 = 기본 없음(09-04 사용자) — 원하면 지정
    ("key.paste_plain", ID_PASTE_PLAIN, "Shift+Alt+X"),
];

/// ★ 창 안 단축키 한 항목(09-08) — (설정 키, Win/Linux 기본, mac 기본). 화면 순서.
///
/// `key.pick_n`은 **조합의 숫자 자리가 1~9로 바뀌는** 패턴이다(`Ctrl+1` = "Ctrl+숫자로 N번째") ·
/// `key.stack_*`는 스택(담아 둔 항목)이 있을 때만 · `key.view_*`/`key.pin`/`key.delete`는 메인창.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowAction {
    /// 설정 키(`key.…`).
    pub key: &'static str,
    /// Windows/Linux 기본 조합.
    pub default_win: &'static str,
    /// macOS 기본 조합(`Win` = ⌘).
    pub default_mac: &'static str,
}

const fn w(
    key: &'static str,
    default_win: &'static str,
    default_mac: &'static str,
) -> WindowAction {
    WindowAction {
        key,
        default_win,
        default_mac,
    }
}

/// 창 안 단축키 전부(09-08 사용자 확정 — 09-01·09-03·09-07에 박혀 있던 내장 키를 설정으로 승격).
pub const WINDOW_ACTIONS: &[WindowAction] = &[
    // 항목 하나 — 팝업은 붙여넣기 · 메인창은 복사.
    w("key.pick", "Enter", "Enter"),
    w("key.pick_plain", "Shift+Enter", "Shift+Enter"),
    w("key.pick_n", "Ctrl+1", "Win+1"),
    // ★ 스택(09-03 ③ · 09-08 확장) — 담기/빼기 · 순차 붙여넣기 4변형(원본/평문 × 줄바꿈 유무).
    w("key.stack_toggle", "Ctrl+Space", "Ctrl+Space"),
    w("key.stack_paste", "Enter", "Enter"),
    w("key.stack_paste_nl", "Alt+Enter", "Alt+Enter"),
    w("key.stack_paste_plain", "Shift+Enter", "Shift+Enter"),
    w(
        "key.stack_paste_plain_nl",
        "Shift+Alt+Enter",
        "Shift+Alt+Enter",
    ),
    // 메인창 항목 관리.
    w("key.pin", "Ctrl+P", "Win+P"),
    w("key.delete", "Delete", "Delete"),
    // 창 공통.
    w("key.settings", "Ctrl+,", "Win+,"),
    w("key.view_rich", "Alt+1", "Alt+1"),
    w("key.view_compact", "Alt+2", "Alt+2"),
    w("key.view_plain", "Alt+3", "Alt+3"),
];

/// 이 설정 키가 **전역** 단축키인가(OS에 등록 · 수식 키 필수) — 아니면 창 안.
#[must_use]
pub fn is_global_key(key: &str) -> bool {
    ACTIONS.iter().any(|(k, _, _)| *k == key)
}

/// 설정 키의 **이 OS 기본 조합**(전역·창 안 공통 · 모르는 키 = 빈 문자열).
#[must_use]
pub fn default_for(key: &str) -> &'static str {
    if let Some((_, _, d)) = ACTIONS.iter().find(|(k, _, _)| *k == key) {
        return d;
    }
    WINDOW_ACTIONS
        .iter()
        .find(|a| a.key == key)
        .map_or("", |a| {
            if cfg!(target_os = "macos") {
                a.default_mac
            } else {
                a.default_win
            }
        })
}

/// 주 키.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    /// 'A'..='Z' · '0'..='9'.
    Char(char),
    /// F1..F24.
    F(u8),
    Space,
    Enter,
    Tab,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    /// ★ 창 안 키용(09-08) — 전역 단축키로도 파싱은 되지만 관례상 쓰지 않는다.
    Backspace,
    /// `,` — 설정 바로가기(`Ctrl+,` · `⌘,`).
    Comma,
}

/// 조합 — 수정 키 + 주 키.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Win / ⌘ / Super.
    pub meta: bool,
    pub key: KeyCode,
}

impl KeyCode {
    /// 토큰(`"C"` · `"F5"` · `"Space"` …) → 키. 대소문자 무관.
    #[must_use]
    pub fn parse(tok: &str) -> Option<Self> {
        let t = tok.trim();
        let up = t.to_ascii_uppercase();
        if up.len() == 1 {
            let c = up.chars().next()?;
            if c.is_ascii_alphanumeric() {
                return Some(Self::Char(c));
            }
            if c == ',' {
                return Some(Self::Comma);
            }
            return None;
        }
        if let Some(n) = up.strip_prefix('F') {
            if let Ok(n) = n.parse::<u8>() {
                if (1..=24).contains(&n) {
                    return Some(Self::F(n));
                }
            }
        }
        Some(match up.as_str() {
            "SPACE" => Self::Space,
            "ENTER" | "RETURN" => Self::Enter,
            "TAB" => Self::Tab,
            "INSERT" | "INS" => Self::Insert,
            "DELETE" | "DEL" => Self::Delete,
            "HOME" => Self::Home,
            "END" => Self::End,
            "PAGEUP" | "PGUP" => Self::PageUp,
            "PAGEDOWN" | "PGDN" => Self::PageDown,
            "UP" => Self::Up,
            "DOWN" => Self::Down,
            "LEFT" => Self::Left,
            "RIGHT" => Self::Right,
            "BACKSPACE" | "BS" => Self::Backspace,
            "COMMA" => Self::Comma,
            _ => return None,
        })
    }

    /// 정규 토큰.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::Char(c) => c.to_string(),
            Self::F(n) => format!("F{n}"),
            Self::Space => "Space".into(),
            Self::Enter => "Enter".into(),
            Self::Tab => "Tab".into(),
            Self::Insert => "Insert".into(),
            Self::Delete => "Delete".into(),
            Self::Home => "Home".into(),
            Self::End => "End".into(),
            Self::PageUp => "PageUp".into(),
            Self::PageDown => "PageDown".into(),
            Self::Up => "Up".into(),
            Self::Down => "Down".into(),
            Self::Left => "Left".into(),
            Self::Right => "Right".into(),
            Self::Backspace => "Backspace".into(),
            Self::Comma => ",".into(),
        }
    }

    /// Windows 가상 키 코드.
    #[must_use]
    pub fn win_vk(self) -> u32 {
        match self {
            Self::Char(c) => c as u32, // 'A'..'Z' = 0x41.. · '0'..'9' = 0x30..
            Self::F(n) => 0x70 + u32::from(n) - 1,
            Self::Space => 0x20,
            Self::Enter => 0x0D,
            Self::Tab => 0x09,
            Self::Insert => 0x2D,
            Self::Delete => 0x2E,
            Self::Home => 0x24,
            Self::End => 0x23,
            Self::PageUp => 0x21,
            Self::PageDown => 0x22,
            Self::Left => 0x25,
            Self::Up => 0x26,
            Self::Right => 0x27,
            Self::Down => 0x28,
            Self::Backspace => 0x08,
            Self::Comma => 0xBC, // VK_OEM_COMMA
        }
    }

    /// mac Carbon 가상 키코드(kVK_ANSI_* · ANSI 배열 기준).
    #[must_use]
    pub fn mac_keycode(self) -> u32 {
        match self {
            Self::Char(c) => match c {
                'A' => 0x00,
                'S' => 0x01,
                'D' => 0x02,
                'F' => 0x03,
                'H' => 0x04,
                'G' => 0x05,
                'Z' => 0x06,
                'X' => 0x07,
                'C' => 0x08,
                'V' => 0x09,
                'B' => 0x0B,
                'Q' => 0x0C,
                'W' => 0x0D,
                'E' => 0x0E,
                'R' => 0x0F,
                'Y' => 0x10,
                'T' => 0x11,
                '1' => 0x12,
                '2' => 0x13,
                '3' => 0x14,
                '4' => 0x15,
                '6' => 0x16,
                '5' => 0x17,
                '9' => 0x19,
                '7' => 0x1A,
                '8' => 0x1C,
                '0' => 0x1D,
                'O' => 0x1F,
                'U' => 0x20,
                'I' => 0x22,
                'P' => 0x23,
                'L' => 0x25,
                'J' => 0x26,
                'K' => 0x28,
                'N' => 0x2D,
                'M' => 0x2E,
                _ => 0xFFFF,
            },
            Self::F(n) => match n {
                1 => 0x7A,
                2 => 0x78,
                3 => 0x63,
                4 => 0x76,
                5 => 0x60,
                6 => 0x61,
                7 => 0x62,
                8 => 0x64,
                9 => 0x65,
                10 => 0x6D,
                11 => 0x67,
                12 => 0x6F,
                _ => 0xFFFF,
            },
            Self::Space => 0x31,
            Self::Enter => 0x24,
            Self::Tab => 0x30,
            Self::Insert => 0x72,
            Self::Delete => 0x75,
            Self::Home => 0x73,
            Self::End => 0x77,
            Self::PageUp => 0x74,
            Self::PageDown => 0x79,
            Self::Left => 0x7B,
            Self::Right => 0x7C,
            Self::Down => 0x7D,
            Self::Up => 0x7E,
            Self::Backspace => 0x33, // kVK_Delete(⌫)
            Self::Comma => 0x2B,
        }
    }

    /// xdg 포털 GlobalShortcuts 트리거 키 이름(소문자).
    #[must_use]
    pub fn portal_key(self) -> String {
        match self {
            Self::Char(c) => c.to_ascii_lowercase().to_string(),
            Self::F(n) => format!("F{n}"),
            Self::Space => "space".into(),
            Self::Enter => "Return".into(),
            Self::Tab => "Tab".into(),
            Self::Insert => "Insert".into(),
            Self::Delete => "Delete".into(),
            Self::Home => "Home".into(),
            Self::End => "End".into(),
            Self::PageUp => "Page_Up".into(),
            Self::PageDown => "Page_Down".into(),
            Self::Up => "Up".into(),
            Self::Down => "Down".into(),
            Self::Left => "Left".into(),
            Self::Right => "Right".into(),
            Self::Backspace => "BackSpace".into(),
            Self::Comma => "comma".into(),
        }
    }
}

impl Hotkey {
    /// `"Shift+Alt+C"` 파싱 — 수정 키 이름은 Ctrl/Control/Shift/Alt/Option/Win/Cmd/Super/Meta(대소문자 무관).
    /// 빈 문자열·주 키 없음·모르는 토큰은 `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let (mut ctrl, mut shift, mut alt, mut meta) = (false, false, false, false);
        let mut key = None;
        for tok in s.split('+') {
            let t = tok.trim();
            if t.is_empty() {
                return None;
            }
            match t.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => ctrl = true,
                "SHIFT" => shift = true,
                "ALT" | "OPTION" => alt = true,
                "WIN" | "CMD" | "COMMAND" | "SUPER" | "META" => meta = true,
                _ => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(KeyCode::parse(t)?);
                }
            }
        }
        Some(Self {
            ctrl,
            shift,
            alt,
            meta,
            key: key?,
        })
    }

    /// 수정 키 상태 + 주 키 토큰으로 만든다(캡처 창).
    #[must_use]
    pub fn from_parts(
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
        key_token: &str,
    ) -> Option<Self> {
        Some(Self {
            ctrl,
            shift,
            alt,
            meta,
            key: KeyCode::parse(key_token)?,
        })
    }

    /// 전역 단축키로 써도 되는가 — 수정 키 하나 이상(F키는 단독 허용).
    #[must_use]
    pub fn is_global_safe(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta || matches!(self.key, KeyCode::F(_))
    }

    /// ★ 창 안 단축키로 써도 되는가(09-08) — 글자·숫자·`,`는 Ctrl/Alt/Win 중 하나 필수(검색창 입력과
    /// 겹친다 · Shift만으로는 `!@#`가 검색어가 된다 — docs/17 M-1). 그 밖(Enter·Delete·Space·F키…)은 맨 키 허용.
    #[must_use]
    pub fn is_window_safe(&self) -> bool {
        match self.key {
            KeyCode::Char(_) | KeyCode::Comma => self.ctrl || self.alt || self.meta,
            _ => true,
        }
    }

    /// ★ 눌린 조합과 **정확히** 같은가(09-08 · 수식 키 넷 전부 비교) — 창 안 단축키 판정.
    #[must_use]
    pub fn matches(&self, ctrl: bool, shift: bool, alt: bool, meta: bool, key: KeyCode) -> bool {
        self.ctrl == ctrl
            && self.shift == shift
            && self.alt == alt
            && self.meta == meta
            && self.key == key
    }

    /// 정규 문자열(저장용 · `Ctrl+Shift+Alt+Win+KEY` 순).
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.shift {
            parts.push("Shift".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        if self.meta {
            parts.push("Win".into());
        }
        parts.push(self.key.token());
        parts.join("+")
    }

    /// 표시용 — mac은 기호(⌃⇧⌥⌘), 그 외는 정규 문자열.
    #[must_use]
    pub fn display(&self, mac: bool) -> String {
        if !mac {
            return self.canonical();
        }
        let mut s = String::new();
        if self.ctrl {
            s.push('⌃');
        }
        if self.shift {
            s.push('⇧');
        }
        if self.alt {
            s.push('⌥');
        }
        if self.meta {
            s.push('⌘');
        }
        s.push_str(&self.key.token());
        s
    }

    /// Windows `RegisterHotKey` 수정 키 비트(MOD_ALT 1 · MOD_CONTROL 2 · MOD_SHIFT 4 · MOD_WIN 8).
    #[must_use]
    pub fn win_mods(&self) -> u32 {
        (u32::from(self.alt))
            | (u32::from(self.ctrl) << 1)
            | (u32::from(self.shift) << 2)
            | (u32::from(self.meta) << 3)
    }

    /// mac Carbon 수정 키 비트(cmdKey 0x100 · shiftKey 0x200 · optionKey 0x800 · controlKey 0x1000).
    #[must_use]
    pub fn mac_mods(&self) -> u32 {
        (if self.meta { 0x100 } else { 0 })
            | (if self.shift { 0x200 } else { 0 })
            | (if self.alt { 0x800 } else { 0 })
            | (if self.ctrl { 0x1000 } else { 0 })
    }

    /// xdg 포털 `preferred_trigger` 문법(`CTRL+SHIFT+v`).
    #[must_use]
    pub fn portal_spec(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("CTRL".into());
        }
        if self.shift {
            parts.push("SHIFT".into());
        }
        if self.alt {
            parts.push("ALT".into());
        }
        if self.meta {
            parts.push("LOGO".into());
        }
        parts.push(self.key.portal_key());
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_canonical_display() {
        let h = Hotkey::parse("shift+alt+c").expect("parse");
        assert_eq!(h.canonical(), "Shift+Alt+C");
        assert_eq!(h.display(true), "⇧⌥C");
        assert_eq!(
            Hotkey::parse("Ctrl+Shift+V").unwrap().canonical(),
            "Ctrl+Shift+V"
        );
        assert_eq!(Hotkey::parse("Cmd+F5").unwrap().canonical(), "Win+F5");
        assert!(Hotkey::parse("").is_none());
        assert!(Hotkey::parse("Ctrl+").is_none());
        assert!(Hotkey::parse("Ctrl+Shift").is_none(), "주 키 없음");
        assert!(Hotkey::parse("Ctrl+A+B").is_none(), "주 키 둘");
        assert!(Hotkey::parse("Ctrl+ㄱ").is_none());
    }

    #[test]
    fn os_codes() {
        let h = Hotkey::parse("Shift+Alt+C").unwrap();
        assert_eq!(h.win_mods(), 0x4 | 0x1);
        assert_eq!(h.key.win_vk(), 0x43);
        assert_eq!(h.mac_mods(), 0x200 | 0x800);
        assert_eq!(h.key.mac_keycode(), 0x08);
        assert_eq!(h.portal_spec(), "SHIFT+ALT+c");
        assert_eq!(
            Hotkey::parse("Ctrl+Shift+V").unwrap().portal_spec(),
            "CTRL+SHIFT+v"
        );
        assert_eq!(KeyCode::parse("f12").unwrap().win_vk(), 0x7B);
        assert_eq!(KeyCode::parse("PageDown").unwrap().mac_keycode(), 0x79);
    }

    #[test]
    fn safety_and_parts() {
        assert!(!Hotkey::parse("C").unwrap().is_global_safe());
        assert!(Hotkey::parse("F9").unwrap().is_global_safe());
        let h = Hotkey::from_parts(false, true, true, false, "X").unwrap();
        assert_eq!(h.canonical(), "Shift+Alt+X");
        assert!(Hotkey::from_parts(true, false, false, false, "Escape").is_none());
        assert_eq!(ACTIONS.len(), 3);
        for (_, _, d) in ACTIONS {
            assert!(d.is_empty() || Hotkey::parse(d).is_some(), "{d}");
        }
    }

    /// ★ 창 안 단축키(09-08) — 기본값은 전부 파싱되고 창 안 규칙을 지키며, 키는 겹치지 않는다.
    #[test]
    fn window_actions_defaults() {
        assert_eq!(WINDOW_ACTIONS.len(), 14);
        for a in WINDOW_ACTIONS {
            for d in [a.default_win, a.default_mac] {
                let h = Hotkey::parse(d).unwrap_or_else(|| panic!("{}: {d}", a.key));
                assert!(h.is_window_safe(), "{}: {d}", a.key);
            }
            assert!(!is_global_key(a.key), "{}", a.key);
            assert!(!default_for(a.key).is_empty(), "{}", a.key);
        }
        let mut keys: Vec<&str> = WINDOW_ACTIONS.iter().map(|a| a.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), WINDOW_ACTIONS.len(), "설정 키 중복");
        assert!(is_global_key("key.open"));
        assert_eq!(default_for("key.open"), "Shift+Alt+C");
        assert_eq!(default_for("key.nope"), "");
    }

    #[test]
    fn window_safe_and_matches() {
        assert!(Hotkey::parse("Enter").unwrap().is_window_safe());
        assert!(Hotkey::parse("Delete").unwrap().is_window_safe());
        assert!(
            !Hotkey::parse("P").unwrap().is_window_safe(),
            "맨 글자 = 검색어"
        );
        assert!(
            !Hotkey::parse("Shift+P").unwrap().is_window_safe(),
            "Shift+글자 = 대문자 검색어"
        );
        assert!(Hotkey::parse("Ctrl+P").unwrap().is_window_safe());
        assert!(!Hotkey::parse(",").unwrap().is_window_safe());
        assert!(Hotkey::parse("Win+,").unwrap().is_window_safe());
        let h = Hotkey::parse("Shift+Alt+Enter").unwrap();
        assert!(h.matches(false, true, true, false, KeyCode::Enter));
        assert!(
            !h.matches(true, true, true, false, KeyCode::Enter),
            "Ctrl이 더 눌림"
        );
        assert!(
            !h.matches(false, true, false, false, KeyCode::Enter),
            "Alt 빠짐"
        );
        // 새 키 토큰.
        assert_eq!(Hotkey::parse("Ctrl+,").unwrap().canonical(), "Ctrl+,");
        assert_eq!(Hotkey::parse("Ctrl+Comma").unwrap().canonical(), "Ctrl+,");
        assert_eq!(
            Hotkey::parse("Alt+Backspace").unwrap().canonical(),
            "Alt+Backspace"
        );
        assert_eq!(KeyCode::Comma.win_vk(), 0xBC);
        assert_eq!(KeyCode::Backspace.mac_keycode(), 0x33);
        assert_eq!(Hotkey::parse("Win+,").unwrap().display(true), "⌘,");
    }
}
