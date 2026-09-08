//! ★ 창 안 키맵(09-08 사용자 — "각 단축키는 설정 가능하게 · 내장 단축키도 단축키 항목에 전부").
//!
//! 설정 `key.*` 중 **창 안** 동작([`nclip_core::hotkey::WINDOW_ACTIONS`])을 파싱해 든 표다. 전역 단축키와
//! 달리 OS에 등록하지 않는다 — 팝업·메인창이 키 이벤트를 받을 때 [`Keymap::is`]로 **정확히**(수식 키 넷
//! 전부) 비교한다. 비교는 **물리 키 자리**([`keycode_token`] — 09-04·09-07 결정: 한글 자판·AZERTY·mac `⌥1`
//! 에서도 같은 자리가 같은 키)로 한다.
//!
//! 표시(`Ctrl+1` 배지 · 상태줄 `Alt+1/2/3` · ⚙ 툴팁 · 푸터 힌트)도 이 표에서 나온다 — 바꾼 값이 곧 화면이다.

use nclip_core::hotkey::{Hotkey, KeyCode, WINDOW_ACTIONS};

/// 눌린 조합 — winit 물리 키 + 수식 상태(창이 `ModifiersChanged`로 추적한 값).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Chord {
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
    pub(crate) alt: bool,
    /// Win / ⌘ / Super.
    pub(crate) meta: bool,
    pub(crate) key: KeyCode,
}

impl Chord {
    /// winit 물리 키 + 수식 상태 → 조합. 수식 키 단독·미지원 키는 `None`.
    pub(crate) fn of(
        pk: &winit::keyboard::PhysicalKey,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) -> Option<Self> {
        let key = KeyCode::parse(keycode_token(pk)?)?;
        Some(Self {
            ctrl,
            shift,
            alt,
            meta,
            key,
        })
    }

    /// 주 키가 숫자 1~9면 그 값.
    fn digit(self) -> Option<usize> {
        match self.key {
            KeyCode::Char(c @ '1'..='9') => Some(usize::from(c as u8 - b'0')),
            _ => None,
        }
    }
}

/// 창 안 키맵 — 설정 키 → 조합(없음 = `None`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Keymap {
    bind: Vec<(&'static str, Option<Hotkey>)>,
}

impl Keymap {
    /// 설정에서 읽는다 — 빈 값·못 읽는 값은 **없음**(기본값으로 되돌리지 않는다: 사용자가 지운 것).
    pub(crate) fn from_conf(conf: &crate::conf::Settings) -> Self {
        Self {
            bind: WINDOW_ACTIONS
                .iter()
                .map(|a| (a.key, Hotkey::parse(conf.state.get(a.key))))
                .collect(),
        }
    }

    /// 변경 감지 서명 — 셸이 박동마다 비교해 바뀌었을 때만 창에 새 표를 준다.
    pub(crate) fn sig(conf: &crate::conf::Settings) -> String {
        let mut s = String::new();
        for a in WINDOW_ACTIONS {
            s.push_str(conf.state.get(a.key).trim());
            s.push('|');
        }
        s
    }

    /// 동작의 조합.
    pub(crate) fn get(&self, key: &str) -> Option<Hotkey> {
        self.bind
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, h)| *h)
    }

    /// 눌린 조합이 이 동작인가(정확 일치 · 없음이면 항상 false).
    pub(crate) fn is(&self, key: &str, c: Chord) -> bool {
        self.get(key)
            .is_some_and(|h| h.matches(c.ctrl, c.shift, c.alt, c.meta, c.key))
    }

    /// ★ N번째 선택(`key.pick_n`) — 조합의 **수식 키**가 같고 주 키가 숫자 1~9면 그 번호.
    ///   (`Ctrl+Alt` 동시 = AltGr는 `Ctrl+1` 설정과 수식 키가 달라 자연히 배제된다.)
    pub(crate) fn pick_n(&self, c: Chord) -> Option<usize> {
        let h = self.get("key.pick_n")?;
        (h.ctrl == c.ctrl && h.shift == c.shift && h.alt == c.alt && h.meta == c.meta)
            .then(|| c.digit())
            .flatten()
    }

    /// 표시 문자열(OS 관례 — mac은 기호) · 없음이면 빈 문자열.
    pub(crate) fn label(&self, key: &str) -> String {
        self.get(key)
            .map(|h| h.display(cfg!(target_os = "macos")))
            .unwrap_or_default()
    }

    /// ★ 번호 배지(09-07 · 1~9행 우측) — `key.pick_n` 조합의 숫자 자리를 `n`으로.
    pub(crate) fn number_badge(&self, n: usize) -> String {
        let Some(h) = self.get("key.pick_n") else {
            return String::new();
        };
        let mut h = h;
        h.key = KeyCode::Char(char::from(b'0' + n as u8));
        h.display(cfg!(target_os = "macos"))
    }

    /// ★ 보기 전환 표기(상태줄) — 셋이 수식 키를 공유하고 주 키만 다르면 `Alt+1/2/3`처럼 접는다.
    pub(crate) fn view_label(&self) -> String {
        let hs: Vec<Hotkey> = ["key.view_rich", "key.view_compact", "key.view_plain"]
            .iter()
            .filter_map(|k| self.get(k))
            .collect();
        let mac = cfg!(target_os = "macos");
        match hs.as_slice() {
            [] => String::new(),
            [a, rest @ ..]
                if rest.iter().all(|b| {
                    (b.ctrl, b.shift, b.alt, b.meta) == (a.ctrl, a.shift, a.alt, a.meta)
                }) =>
            {
                let head = a.display(mac);
                let prefix = &head[..head.len() - a.key.token().len()];
                let keys: Vec<String> = hs.iter().map(|h| h.key.token()).collect();
                format!("{prefix}{}", keys.join("/"))
            }
            _ => hs
                .iter()
                .map(|h| h.display(mac))
                .collect::<Vec<_>>()
                .join(" · "),
        }
    }
}

/// 문구의 `{}`를 단축키 표기로 — 표기가 비면(없음) ` ({})` 괄호째 뺀다(툴팁 "고정/해제 (Ctrl+P)").
pub(crate) fn with_key(template: &str, label: &str) -> String {
    if label.is_empty() {
        template.replace(" ({})", "").replacen("{}", "", 1)
    } else {
        template.replacen("{}", label, 1)
    }
}

/// winit 물리 키 → 단축키 토큰(09-04 캡처 · 09-08 창 안 공용) — 글자·숫자·F키·편집/이동 키 ·
/// Enter·Backspace·`,`. 수정 키 단독·그 밖은 `None`.
pub(crate) fn keycode_token(pk: &winit::keyboard::PhysicalKey) -> Option<&'static str> {
    use winit::keyboard::{KeyCode as K, PhysicalKey};
    let PhysicalKey::Code(code) = pk else {
        return None;
    };
    Some(match code {
        K::KeyA => "A",
        K::KeyB => "B",
        K::KeyC => "C",
        K::KeyD => "D",
        K::KeyE => "E",
        K::KeyF => "F",
        K::KeyG => "G",
        K::KeyH => "H",
        K::KeyI => "I",
        K::KeyJ => "J",
        K::KeyK => "K",
        K::KeyL => "L",
        K::KeyM => "M",
        K::KeyN => "N",
        K::KeyO => "O",
        K::KeyP => "P",
        K::KeyQ => "Q",
        K::KeyR => "R",
        K::KeyS => "S",
        K::KeyT => "T",
        K::KeyU => "U",
        K::KeyV => "V",
        K::KeyW => "W",
        K::KeyX => "X",
        K::KeyY => "Y",
        K::KeyZ => "Z",
        K::Digit0 | K::Numpad0 => "0",
        K::Digit1 | K::Numpad1 => "1",
        K::Digit2 | K::Numpad2 => "2",
        K::Digit3 | K::Numpad3 => "3",
        K::Digit4 | K::Numpad4 => "4",
        K::Digit5 | K::Numpad5 => "5",
        K::Digit6 | K::Numpad6 => "6",
        K::Digit7 | K::Numpad7 => "7",
        K::Digit8 | K::Numpad8 => "8",
        K::Digit9 | K::Numpad9 => "9",
        K::F1 => "F1",
        K::F2 => "F2",
        K::F3 => "F3",
        K::F4 => "F4",
        K::F5 => "F5",
        K::F6 => "F6",
        K::F7 => "F7",
        K::F8 => "F8",
        K::F9 => "F9",
        K::F10 => "F10",
        K::F11 => "F11",
        K::F12 => "F12",
        K::Space => "Space",
        K::Enter | K::NumpadEnter => "Enter",
        K::Tab => "Tab",
        K::Backspace => "Backspace",
        K::Comma => ",",
        K::Insert => "Insert",
        K::Delete => "Delete",
        K::Home => "Home",
        K::End => "End",
        K::PageUp => "PageUp",
        K::PageDown => "PageDown",
        K::ArrowUp => "Up",
        K::ArrowDown => "Down",
        K::ArrowLeft => "Left",
        K::ArrowRight => "Right",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn km(pairs: &[(&'static str, &str)]) -> Keymap {
        Keymap {
            bind: WINDOW_ACTIONS
                .iter()
                .map(|a| {
                    let v = pairs
                        .iter()
                        .find(|(k, _)| *k == a.key)
                        .map_or(nclip_core::hotkey::default_for(a.key), |(_, v)| v);
                    (a.key, Hotkey::parse(v))
                })
                .collect(),
        }
    }

    fn chord(s: &str) -> Chord {
        let h = Hotkey::parse(s).expect("조합 파싱");
        Chord {
            ctrl: h.ctrl,
            shift: h.shift,
            alt: h.alt,
            meta: h.meta,
            key: h.key,
        }
    }

    #[test]
    fn exact_match_and_pick_n() {
        let k = km(&[]);
        assert!(k.is("key.stack_paste_nl", chord("Alt+Enter")));
        assert!(
            !k.is("key.stack_paste_nl", chord("Ctrl+Alt+Enter")),
            "정확 일치"
        );
        assert!(k.is("key.stack_paste_plain_nl", chord("Shift+Alt+Enter")));
        assert!(k.is("key.pick", chord("Enter")));
        assert!(!k.is("key.pick", chord("Shift+Enter")));
        let n = if cfg!(target_os = "macos") {
            "Win+3"
        } else {
            "Ctrl+3"
        };
        assert_eq!(k.pick_n(chord(n)), Some(3));
        assert_eq!(k.pick_n(chord("Ctrl+Alt+3")), None, "AltGr 배제");
        assert_eq!(k.pick_n(chord("Ctrl+0")), None);
        // 지운 키는 어디에도 안 맞는다.
        let k2 = km(&[("key.pick_n", ""), ("key.pin", "")]);
        assert_eq!(k2.pick_n(chord(n)), None);
        assert!(!k2.is("key.pin", chord("Ctrl+P")));
        assert_eq!(k2.label("key.pin"), "");
    }

    #[test]
    fn labels() {
        let k = km(&[]);
        if cfg!(target_os = "macos") {
            assert_eq!(k.number_badge(4), "⌘4");
            assert_eq!(k.view_label(), "⌥1/2/3");
            assert_eq!(k.label("key.settings"), "⌘,");
        } else {
            assert_eq!(k.number_badge(4), "Ctrl+4");
            assert_eq!(k.view_label(), "Alt+1/2/3");
            assert_eq!(k.label("key.settings"), "Ctrl+,");
        }
        let k = km(&[("key.view_compact", "Ctrl+Shift+2")]);
        assert!(k.view_label().contains(" · "), "{}", k.view_label());
        let k = km(&[
            ("key.view_rich", ""),
            ("key.view_compact", ""),
            ("key.view_plain", ""),
        ]);
        assert_eq!(k.view_label(), "");
        assert_eq!(with_key("고정/해제 ({})", "Ctrl+P"), "고정/해제 (Ctrl+P)");
        assert_eq!(with_key("고정/해제 ({})", ""), "고정/해제");
        assert_eq!(with_key("{} 원본 · Esc", ""), " 원본 · Esc");
    }

    #[test]
    fn token_of_physical_keys() {
        use winit::keyboard::{KeyCode as K, PhysicalKey};
        assert_eq!(
            keycode_token(&PhysicalKey::Code(K::NumpadEnter)),
            Some("Enter")
        );
        assert_eq!(keycode_token(&PhysicalKey::Code(K::Comma)), Some(","));
        assert_eq!(keycode_token(&PhysicalKey::Code(K::ControlLeft)), None);
        assert_eq!(
            Chord::of(&PhysicalKey::Code(K::Digit7), true, false, false, false),
            Some(chord("Ctrl+7"))
        );
    }
}
