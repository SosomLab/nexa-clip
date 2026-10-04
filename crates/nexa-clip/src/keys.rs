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

/// ★ 키 진단 로그(09-27) — `NEXA_CLIP_KEYDIAG=1`이면 팝업이 받는 `KeyboardInput`·`Ime` 이벤트를 stdout에 전부 찍는다
/// (X 서버 사실은 `scripts/linux-keyprobe`가 · winit이 앱에 무엇을 넘겼는가는 이것만이 안다 — xev·raw 로거로는
/// 볼 수 없다: XI2 마스크가 걸린 창의 core KeyPress는 타 클라이언트에 안 가고 XKB 오토리피트는 raw 이벤트가 없다).
pub(crate) fn diag() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NEXA_CLIP_KEYDIAG").is_some_and(|v| !v.is_empty()))
}

/// X 키코드로 본 수식키(evdev + 8) — Ctrl·Shift·Alt·Super 좌우.
#[cfg(target_os = "linux")]
const X_MODIFIER_KEYCODES: [u32; 8] = [37, 50, 62, 64, 105, 108, 133, 134];

/// 수식키인가 — 잔향 집합에서 글자 키와 가른다.
fn is_modifier(pk: &winit::keyboard::PhysicalKey) -> bool {
    use winit::keyboard::{KeyCode as K, PhysicalKey};
    matches!(
        pk,
        PhysicalKey::Code(
            K::ShiftLeft
                | K::ShiftRight
                | K::ControlLeft
                | K::ControlRight
                | K::AltLeft
                | K::AltRight
                | K::SuperLeft
                | K::SuperRight
        )
    )
}

/// ★ 포커스 잔향 게이트(09-27 · Linux `c` 스톰 · T-15d 근인 확정) — 창이 포커스를 받을 때 **이미 눌려 있던**
/// 키는 그 키의 **해제가 올 때까지** 이 창의 입력이 아니다.
///
/// winit은 X11(`FocusIn` → `handle_pressed_keys`)·Windows(`WM_SETFOCUS` → `synthesize_kbd_state`)에서 눌려 있는
/// 키 전부를 `is_synthetic: true` **Pressed**로 먼저 알려 준다(mac은 없음 = 게이트 무동작). 그 집합이 곧 "단축키를
/// 누른 손"이다. 종전(09-05)엔 합성 이벤트만 버리고 **첫 진짜 누름을 보면 잔향이 끝났다고 봤는데**, GNOME(mutter
/// 50 · XWayland 24.1)에서는 실측상 ① 팝업이 포커스를 받는 순간 XWayland 키 상태에 `c`가 눌림으로 등록되고(raw 이벤트
/// 없이) ② 사용자가 **수식키를 쥔 채 `c`를 먼저 떼면 mutter가 그 해제를 삼켜** X 서버에 안 보낸다 → `c`가 **영구
/// 고착** → X 서버 오토리피트가 무한(`cccc…`). 첫 리피트가 winit에는 `repeat: false`(소프트웨어 판정 · 포커스 뒤
/// 첫 누름)로 와서 종전 게이트를 그대로 통과했다. 포털 RemoteDesktop으로 해제만 주입해도 풀리지 않는다(실측) —
/// 그래서 **해제를 볼 때까지 그 키를 통째로 무시**한다. 실기 하네스 = `scripts/linux-keyprobe`.
///
/// 알려진 한계: 고착 상태에서 사용자가 `c`를 **다시** 누르면 X 서버는 그것을 리피트로 보내(해제가 와야 풀림)
/// 첫 `c` 한 글자가 빠진다. 두 번째부터 정상 · 팝업이 닫히면 XWayland가 전부 해제한다(실측).
#[derive(Clone, Debug, Default)]
pub(crate) struct FocusResidue {
    held: Vec<winit::keyboard::PhysicalKey>,
}

impl FocusResidue {
    /// 창을 열 때·포커스를 잃을 때 — 잔향 집합 초기화.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// 키 이벤트 하나를 이 창이 **처리해도 되는가**. 합성 이벤트는 집합만 갱신하고 항상 `false`.
    /// 잔향 키의 진짜 Pressed(리피트든 아니든)는 `false` · 그 키의 Released가 오면 집합에서 빠지고 `true`.
    pub(crate) fn admit(
        &mut self,
        pk: &winit::keyboard::PhysicalKey,
        pressed: bool,
        synthetic: bool,
    ) -> bool {
        if synthetic {
            if pressed {
                if !self.held.contains(pk) {
                    self.held.push(*pk);
                }
            } else {
                self.held.retain(|k| k != pk);
            }
            return false;
        }
        if pressed {
            !self.held.contains(pk)
        } else {
            self.held.retain(|k| k != pk);
            true
        }
    }

    /// 외부 조회(X 서버 `QueryKeymap`)로 안 "지금 눌려 있는 키"를 잔향으로 묶는다 — 이미 있으면 그대로.
    /// (Linux 프로브와 테스트만 부른다 — 다른 OS 산출물에서는 죽은 코드가 맞다.)
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) fn mark_held(&mut self, pk: winit::keyboard::PhysicalKey) {
        if !self.held.contains(&pk) {
            self.held.push(pk);
        }
    }

    /// ★ X 서버 키 상태 프로브(Linux · 09-27) — 서버가 눌렸다고 믿는 키 전부를 잔향으로. 반환 = 새로 묶인 수.
    ///   winit 합성 집합이 비어 있는 이유는 [`nclip_plat::keystate_x11`] 참조. 다른 OS = 무동작(0).
    pub(crate) fn probe_x11(&mut self) -> usize {
        #[cfg(target_os = "linux")]
        {
            use winit::platform::scancode::PhysicalKeyExtScancode as _;
            let Some(down) = nclip_plat::keystate_x11::keys_down() else {
                return 0;
            };
            let before = self.held.len();
            for kc in down {
                // X keycode = evdev 스캔코드 + 8 (winit Linux 백엔드의 `from_scancode` 규약).
                let pk = winit::keyboard::PhysicalKey::from_scancode(kc.saturating_sub(8));
                if !matches!(pk, winit::keyboard::PhysicalKey::Unidentified(_)) {
                    self.mark_held(pk);
                }
            }
            self.held.len() - before
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
    }

    /// ★ 잔향에 **글자 키**(수식키가 아닌 키)가 남아 있는가(10-04) — 있으면 IME 입력도 잔향으로 본다.
    ///
    /// 한글 입력 상태에서는 고착된 글자 키의 오토리피트가 `KeyboardInput`이 아니라 **IME 이벤트**
    /// (`Preedit`·`Commit` "ㅊ")로 온다 — 키 게이트만으로는 못 막는다(10-04 사용자 재신고 · 하네스 재현).
    pub(crate) fn has_letters(&self) -> bool {
        self.held.iter().any(|k| !is_modifier(k))
    }

    /// ★ X 서버가 **이미 뗐다고** 아는 잔향 키를 집합에서 뺀다(Linux · 10-04) — 입력기가 해제 이벤트를
    ///   삼켜도 잔향이 풀리게. 반환 = 뺀 수. 다른 OS = 무동작(0).
    pub(crate) fn sync_released_x11(&mut self) -> usize {
        #[cfg(target_os = "linux")]
        {
            use winit::platform::scancode::PhysicalKeyExtScancode as _;
            let Some(down) = nclip_plat::keystate_x11::keys_down() else {
                return 0;
            };
            let before = self.held.len();
            self.held
                .retain(|k| k.to_scancode().is_some_and(|sc| down.contains(&(sc + 8))));
            before - self.held.len()
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
    }

    /// ★ **고착된 글자 키**의 evdev 코드(Linux · 10-04) — X 서버에는 눌려 있는데 수식키는 전부 떼어진 잔향 글자 키.
    ///
    /// 단축키(`Shift+Alt+C`)를 누른 손이 수식키까지 다 뗐는데 글자 키만 서버에 남아 있다 = mutter가 그 해제를
    /// 삼킨 고착이다(09-27 실측). 수식키가 아직 눌려 있으면 사용자가 쥐고 있는 중이라 빈 목록.
    /// 호출자는 이 키들에 **누름 + 뗌**을 주입해 푼다(뗌만으로는 안 풀린다 · 실측). 다른 OS = 빈 목록.
    pub(crate) fn stuck_letters_x11(&self) -> Vec<i32> {
        #[cfg(target_os = "linux")]
        {
            use winit::platform::scancode::PhysicalKeyExtScancode as _;
            let Some(down) = nclip_plat::keystate_x11::keys_down() else {
                return Vec::new();
            };
            let mods_down = self
                .held
                .iter()
                .filter(|k| is_modifier(k))
                .filter_map(|k| k.to_scancode())
                .any(|sc| down.contains(&(sc + 8)))
                || down.iter().any(|kc| X_MODIFIER_KEYCODES.contains(kc));
            if mods_down {
                return Vec::new();
            }
            self.held
                .iter()
                .filter(|k| !is_modifier(k))
                .filter_map(|k| k.to_scancode())
                .filter(|sc| down.contains(&(sc + 8)))
                .filter_map(|sc| i32::try_from(sc).ok())
                .collect()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Vec::new()
        }
    }

    /// 잔향으로 묶인 키 수(진단·테스트).
    pub(crate) fn len(&self) -> usize {
        self.held.len()
    }
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

    /// ★ 10-04 — 잔향에 글자 키가 있으면 IME 입력도 잔향이다(수식키만 남았으면 아니다).
    #[test]
    fn residue_letters_gate_ime() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let mut r = FocusResidue::default();
        assert!(!r.has_letters());
        r.mark_held(PhysicalKey::Code(KeyCode::ShiftLeft));
        r.mark_held(PhysicalKey::Code(KeyCode::AltLeft));
        assert!(!r.has_letters(), "수식키만 = 글자 잔향 없음");
        r.mark_held(PhysicalKey::Code(KeyCode::KeyC));
        assert!(r.has_letters());
        // 해제가 오면 풀린다.
        assert!(r.admit(&PhysicalKey::Code(KeyCode::KeyC), false, false));
        assert!(!r.has_letters());
    }

    // ── FocusResidue(09-27 · Linux `c` 스톰) ──
    use winit::keyboard::{KeyCode as WK, PhysicalKey as PK};

    /// T1 실측 재현: 포커스 때 Shift·Alt·c 합성 Pressed → X 오토리피트(첫 것은 repeat=false)가 무한 · 해제는 영영 없음.
    #[test]
    fn residue_swallows_stuck_key_storm_until_release() {
        let mut r = FocusResidue::default();
        for k in [WK::ShiftLeft, WK::AltLeft, WK::KeyC] {
            assert!(!r.admit(&PK::Code(k), true, true), "합성은 처리하지 않는다");
        }
        assert_eq!(r.len(), 3);
        for _ in 0..200 {
            assert!(
                !r.admit(&PK::Code(WK::KeyC), true, false),
                "고착 리피트는 전부 버린다"
            );
        }
        // 수식키 해제는 정상 도착(실측) → 집합에서 빠진다 · 처리 자체는 허용(팝업은 Released를 쓰지 않는다).
        assert!(r.admit(&PK::Code(WK::AltLeft), false, false));
        assert!(r.admit(&PK::Code(WK::ShiftLeft), false, false));
        assert_eq!(r.len(), 1);
        // 잔향과 무관한 키는 즉시 입력이다(검색은 계속 된다).
        assert!(r.admit(&PK::Code(WK::KeyA), true, false));
        // 사용자가 c를 다시 눌러 뗀 뒤(T2 실측 — 누름/뗌이 와야 풀린다) 부터 c가 산다.
        assert!(
            !r.admit(&PK::Code(WK::KeyC), true, false),
            "고착 중 재누름은 서버 리피트 = 한 글자 빠짐(알려진 한계)"
        );
        assert!(r.admit(&PK::Code(WK::KeyC), false, false));
        assert!(r.admit(&PK::Code(WK::KeyC), true, false));
        assert_eq!(r.len(), 0);
    }

    /// ★ 09-27 계측: 포커스 때 합성 Pressed가 **없다**(X 서버 등록이 FocusIn 뒤) — 프로브(`mark_held`)가 넣은
    /// 키도 같은 규칙: 첫 팬텀(repeat=false)부터 전부 버리고, 해제가 오면 산다.
    #[test]
    fn residue_from_probe_swallows_phantom_first_press() {
        let mut r = FocusResidue::default();
        r.mark_held(PK::Code(WK::ShiftLeft));
        r.mark_held(PK::Code(WK::KeyC));
        r.mark_held(PK::Code(WK::KeyC)); // 중복 무해
        assert_eq!(r.len(), 2);
        assert!(
            !r.admit(&PK::Code(WK::KeyC), true, false),
            "팬텀 첫 누름(repeat=false)"
        );
        assert!(!r.admit(&PK::Code(WK::KeyC), true, false), "리피트");
        assert!(r.admit(&PK::Code(WK::ShiftLeft), false, false));
        assert!(
            r.admit(&PK::Code(WK::KeyA), true, false),
            "다른 키는 즉시 입력"
        );
        assert!(r.admit(&PK::Code(WK::KeyC), false, false));
        assert!(r.admit(&PK::Code(WK::KeyC), true, false));
        assert_eq!(r.len(), 0);
    }

    /// T4 실측: 수식키를 먼저 떼면 c 해제가 정상 도착 → 그 뒤 c는 입력.
    #[test]
    fn residue_released_key_becomes_input() {
        let mut r = FocusResidue::default();
        r.admit(&PK::Code(WK::KeyC), true, true);
        assert!(!r.admit(&PK::Code(WK::KeyC), true, false));
        assert!(r.admit(&PK::Code(WK::KeyC), false, false));
        assert!(r.admit(&PK::Code(WK::KeyC), true, false));
    }

    /// 포커스 상실의 합성 Released(winit X11 FocusOut)·clear 는 집합을 비운다 — mac처럼 합성이 없으면 게이트는 무동작.
    #[test]
    fn residue_clears_on_synthetic_release_and_clear() {
        let mut r = FocusResidue::default();
        r.admit(&PK::Code(WK::KeyV), true, true);
        assert!(!r.admit(&PK::Code(WK::KeyV), false, true));
        assert!(r.admit(&PK::Code(WK::KeyV), true, false));
        r.admit(&PK::Code(WK::KeyV), true, true);
        r.clear();
        assert!(r.admit(&PK::Code(WK::KeyV), true, false));
        let mut none = FocusResidue::default();
        assert!(none.admit(&PK::Code(WK::KeyV), true, false));
    }
}
