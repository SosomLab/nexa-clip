//! ★ 전역 단축키 모델(09-04 사용자 — "단축키 지정 화면 · 캡처 창 · 여러 기능에 지정").
//!
//! 설정에는 **공통 문자열**로 저장한다(`Ctrl+Shift+Alt+Win+C` 순서 · 대소문자 무관 파싱). 표시는 OS 관례(mac은 ⌃⇧⌥⌘).
//! OS 등록에 필요한 숫자(Windows VK · mac Carbon 키코드 · 포털 spec)는 여기서 낸다 — 플랫폼 코드는 값만 받는다.
//!
//! 동작 id(플랫폼 이벤트가 되돌려 주는 번호): 1 = 퀵 팝업 · 2 = 퀵 팝업(보조) · 3 = 평문 붙여넣기.

/// 단축키 동작 id — 설정 키와 1:1.
pub const ID_OPEN: u32 = 1;
pub const ID_OPEN_ALT: u32 = 2;
pub const ID_PASTE_PLAIN: u32 = 3;

/// (설정 키, 동작 id, 기본 조합) — 화면 순서.
pub const ACTIONS: &[(&str, u32, &str)] = &[
    ("key.open", ID_OPEN, "Shift+Alt+C"),
    ("key.open_alt", ID_OPEN_ALT, "Ctrl+Shift+V"),
    ("key.paste_plain", ID_PASTE_PLAIN, "Shift+Alt+X"),
];

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
            assert!(Hotkey::parse(d).is_some(), "{d}");
        }
    }
}
