//! ★ 설정 ↔ JSON(11-03 · 사용자 "설정 가져오기 · 내보내기 · JSON 직접 수정") — 키 `a.b.c`를 **객체 계층**
//! `{"a":{"b":{"c":…}}}`로 내보내고, 파일이 저장되면 호스트가 다시 읽어 **바뀐 키만** 반영한다.
//! 원본 = nexa-sql `crates/nsql-settings/src/json.rs`(09-15) 이식 — 외부 crate 0(필요한 만큼의 JSON 파서·작성기).
//!
//! - 값 타입: `Number` → 숫자 · `Toggle` → true/false · 그 외 → 문자열. 값 키가 있는 레지스트리 항목 전부(기본값 포함) ·
//!   화면 밖 영속 키([`HIDDEN_KEYS`] — 창 위치 등)는 내보내지 않는다(사용자 설정이 아니다 · 가져올 때는 받는다).
//! - 가져오기: JSON에 **있는** 키만 적용(없는 키는 그대로) · `_`로 시작하는 키는 무시 · 모르는 키·틀린 값은 보고만 하고
//!   나머지는 적용. 검증 = [`SettingsState::set_by_name`]과 같은 종류 검사 + [`validate`](범위).

use std::collections::BTreeMap;

use crate::settings::{registry, validate, value_valid, SettingKind, SettingsState, HIDDEN_KEYS};

/// 최소 JSON 값.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// 설정 값 문자열로(객체는 값이 아니다 → None).
    fn scalar_text(&self) -> Option<String> {
        match self {
            Json::Null => Some(String::new()),
            Json::Bool(b) => Some(if *b { "on".into() } else { "off".into() }),
            Json::Num(n) => Some(if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }),
            Json::Str(s) => Some(s.clone()),
            Json::Arr(items) => Some(
                items
                    .iter()
                    .filter_map(Json::scalar_text)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            Json::Obj(_) => None,
        }
    }
}

/// JSON 텍스트 파싱(오류 = 위치 포함 메시지).
pub fn parse(text: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing characters at {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at {}", c as char, self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected '{}' at {}", c as char, self.i)),
            None => Err("unexpected end".into()),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(format!("invalid literal at {}", self.i))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(
                self.b[self.i],
                b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
            )
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
        s.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("bad number '{s}' at {start}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("unterminated string".into());
            };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err("bad escape".into());
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self.b.get(self.i..self.i + 4).ok_or("bad \\u escape")?;
                            let cp = u32::from_str_radix(
                                std::str::from_utf8(hex).map_err(|e| e.to_string())?,
                                16,
                            )
                            .map_err(|e| e.to_string())?;
                            self.i += 4;
                            // 대리쌍
                            let ch = if (0xD800..0xDC00).contains(&cp)
                                && self.b.get(self.i) == Some(&b'\\')
                                && self.b.get(self.i + 1) == Some(&b'u')
                            {
                                let hex2 =
                                    self.b.get(self.i + 2..self.i + 6).ok_or("bad surrogate")?;
                                let lo = u32::from_str_radix(
                                    std::str::from_utf8(hex2).map_err(|e| e.to_string())?,
                                    16,
                                )
                                .map_err(|e| e.to_string())?;
                                self.i += 6;
                                0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                cp
                            };
                            out.push(char::from_u32(ch).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(format!("bad escape at {}", self.i)),
                    }
                }
                _ => {
                    // UTF-8 바이트 → 문자(멀티바이트는 통째로).
                    let start = self.i - 1;
                    let len = utf8_len(c);
                    let end = (start + len).min(self.b.len());
                    out.push_str(std::str::from_utf8(&self.b[start..end]).unwrap_or("\u{FFFD}"));
                    self.i = end;
                }
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected ',' or ']' at {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(items));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            self.expect(b':')?;
            self.ws();
            let v = self.value()?;
            items.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(items));
                }
                _ => return Err(format!("expected ',' or '}}' at {}", self.i)),
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// 중첩 트리(내보내기용).
#[derive(Default)]
struct Node {
    leaf: Option<String>,
    kids: BTreeMap<String, Node>,
}

/// 값 키의 JSON 표현 — 숫자 항목은 숫자, 토글은 불, 나머지는 문자열(글꼴 크기 짝 키처럼 종류가 다른 파생 키도 문자열).
fn json_value_for(kind: SettingKind, key: &str, main_key: &str, v: &str) -> String {
    if key != main_key {
        return escape(v);
    }
    match kind {
        SettingKind::Number { .. } => v
            .parse::<i64>()
            .map_or_else(|_| escape(v), |n| n.to_string()),
        SettingKind::Toggle => (v == "on").to_string(),
        _ => escape(v),
    }
}

fn write_node(out: &mut String, node: &Node, depth: usize) {
    let ind = "  ".repeat(depth);
    out.push_str("{\n");
    let n = node.kids.len();
    for (i, (k, child)) in node.kids.iter().enumerate() {
        out.push_str(&ind);
        out.push_str("  ");
        out.push_str(&escape(k));
        out.push_str(": ");
        match &child.leaf {
            Some(v) if child.kids.is_empty() => out.push_str(v),
            _ => write_node(out, child, depth + 1),
        }
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&ind);
    out.push('}');
}

/// 전체 설정을 객체 계층 JSON으로(현재 값 · 기본값 포함 · 키 사전순 · 2칸 들여쓰기).
#[must_use]
pub fn to_json(s: &SettingsState) -> String {
    let mut root = Node::default();
    root.kids.insert(
        "_comment".into(),
        Node {
            leaf: Some(escape(
                "Nexa Clip settings - edit and save; the app applies changed keys immediately while this file is being watched. Unknown keys are ignored.",
            )),
            kids: BTreeMap::new(),
        },
    );
    for e in registry() {
        for (k, _) in e.default_values() {
            let v = s.get(k);
            let mut cur = &mut root;
            for seg in k.split('.') {
                cur = cur.kids.entry(seg.to_string()).or_default();
            }
            cur.leaf = Some(json_value_for(e.kind, k, e.key, v));
        }
    }
    let mut out = String::new();
    write_node(&mut out, &root, 0);
    out.push('\n');
    out
}

/// 가져오기 결과.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Import {
    /// 값이 실제로 바뀐 키(레지스트리의 `'static` 키 — 호스트가 그대로 즉시 적용 경로에 넣는다).
    pub changed: Vec<&'static str>,
    /// 레지스트리에 없는 키(무시).
    pub unknown: Vec<String>,
    /// 종류·범위에 맞지 않는 값(키, 값) — 무시.
    pub invalid: Vec<(String, String)>,
}

fn flatten(prefix: &str, v: &Json, out: &mut Vec<(String, Json)>) {
    match v {
        Json::Obj(items) => {
            for (k, child) in items {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, child, out);
            }
        }
        other => out.push((prefix.to_string(), other.clone())),
    }
}

/// 키 이름 → (`'static` 키, 종류) — 값 키 전부(글꼴 크기 짝 키 포함) · 화면 밖 영속 키는 자유 문자열.
fn lookup(key: &str) -> Option<(&'static str, Option<SettingKind>)> {
    if let Some(k) = HIDDEN_KEYS.iter().find(|k| **k == key) {
        return Some((k, None));
    }
    for e in registry() {
        for (k, _) in e.default_values() {
            if k == key {
                return Some((k, Some(e.kind)));
            }
        }
    }
    None
}

/// JSON 텍스트를 설정에 적용(메모리) — 있는 키만 · 바뀐 키 목록. 파일 쓰기·즉시 반영은 호스트 몫.
pub fn import_json(s: &mut SettingsState, text: &str) -> Result<Import, String> {
    let v = parse(text)?;
    let mut flat = Vec::new();
    flatten("", &v, &mut flat);
    let mut r = Import::default();
    for (k, jv) in flat {
        if k.starts_with('_') || k.split('.').any(|seg| seg.starts_with('_')) {
            continue;
        }
        let Some((key, kind)) = lookup(&k) else {
            r.unknown.push(k);
            continue;
        };
        let Some(raw) = jv.scalar_text() else {
            r.unknown.push(k);
            continue;
        };
        // 값 키 전부 같은 종류 검사(글꼴 크기 짝 키는 소비처가 관용 파싱 — Text처럼 통과).
        let ok = match kind {
            Some(kind) if key == registry_key_of(key) => {
                value_valid(kind, &raw) && validate(key, &raw).is_ok()
            }
            _ => true,
        };
        if !ok {
            r.invalid.push((k, raw));
            continue;
        }
        if s.get(key) != raw {
            s.set(key, raw);
            r.changed.push(key);
        }
    }
    Ok(r)
}

/// 값 키가 속한 항목의 대표 키(파생 키면 대표 키가 다르다).
fn registry_key_of(value_key: &'static str) -> &'static str {
    registry()
        .iter()
        .find(|e| e.default_values().iter().any(|(k, _)| *k == value_key))
        .map_or(value_key, |e| e.key)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trip_and_unicode() {
        let v = parse(r#"{"a": {"b": [1, 2.5, "x\ny", true, null]}, "k": "한글 é"}"#).unwrap();
        match v {
            Json::Obj(items) => {
                assert_eq!(items[0].0, "a");
                assert_eq!(items[1].1, Json::Str("한글 é".into()));
            }
            _ => panic!(),
        }
        assert!(parse("{").is_err());
        assert!(parse("{\"a\":1} x").is_err());
    }

    #[test]
    fn export_is_nested_and_import_applies_changed_keys_only() {
        let mut s = SettingsState::with_defaults();
        let text = to_json(&s);
        assert!(text.contains("\"ui\": {"), "{text}");
        assert!(text.contains("\"toast_ms\": 3000"), "숫자 항목은 숫자");
        assert!(text.contains("\"fast\": true"), "토글은 불");
        assert!(
            !text.contains("\"win_x\""),
            "화면 밖 영속 키는 내보내지 않는다"
        );
        // 다시 읽으면 바뀐 키 0.
        let r = import_json(&mut s, &text).unwrap();
        assert!(r.changed.is_empty(), "{r:?}");
        assert!(r.unknown.is_empty());
        assert!(r.invalid.is_empty());
        // 일부만 바꾼 JSON — 바뀐 키 · 모르는 키 · 범위 밖 값 · 종류 밖 값.
        let r = import_json(
            &mut s,
            r#"{"ui": {"toast_ms": 5000, "theme": "dark", "tray_recent_n": 99, "toast_progress": "maybe"}, "nope": {"x": 1}}"#,
        )
        .unwrap();
        assert_eq!(r.changed, vec!["ui.toast_ms", "ui.theme"]);
        assert_eq!(r.unknown, vec!["nope.x"]);
        assert_eq!(
            r.invalid,
            vec![
                ("ui.tray_recent_n".to_string(), "99".to_string()),
                ("ui.toast_progress".to_string(), "maybe".to_string()),
            ]
        );
        assert_eq!(s.get("ui.toast_ms"), "5000");
        assert_eq!(s.get("ui.theme"), "dark");
        assert_eq!(s.get("ui.tray_recent_n"), "8", "범위 밖은 그대로");
    }

    #[test]
    fn underscore_keys_are_ignored_and_hidden_keys_import() {
        let mut s = SettingsState::with_defaults();
        let r = import_json(
            &mut s,
            r#"{"_comment": "x", "ui": {"_note": 1, "win_x": "120"}}"#,
        )
        .unwrap();
        assert_eq!(r.changed, vec!["ui.win_x"]);
        assert!(r.unknown.is_empty());
        assert_eq!(s.get("ui.win_x"), "120");
    }
}
