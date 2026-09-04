//! ★ 검색 매처(09-04 사용자 — "nexa-app을 치면 4·5번이 빠진다 · 정규식도 선택하게").
//!
//! 종전엔 메인·팝업이 **라벨(첫 줄 44자)** 부분 문자열만 봤다. 이제 [`Matcher`]가 **라벨 + 본문 전체**를 보고,
//! 설정 `find.mode`(정확히 · 유사 · 정규식)를 따른다. 대소문자는 무시한다.
//!
//! - **정확히**(`Exact`): 질의 전체가 부분 문자열로 들어 있다.
//! - **유사**(`Fuzzy`): 띄어쓴 단어가 **전부**(순서 무관) 들어 있다 — `nexa app pdb`.
//! - **정규식**(`Regex`): 자체 백트래킹 엔진(DR-8 — 외부 crate 0). 지원: 리터럴 · `.` · `* + ?` · `{n} {n,} {n,m}` ·
//!   `[abc] [a-z] [^…]` · `\d \w \s \D \W \S \b` · `^ $` · `( … | … )`. 컴파일 실패면 **정확히**로 폴백하고
//!   [`Matcher::error`]에 이유를 남긴다. 폭주 방어: 매칭 단계 20만 회 상한(넘으면 불일치).

use std::cell::Cell;

/// 검색 방식 — 설정 `find.mode` 값과 1:1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Exact,
    Fuzzy,
    Regex,
}

impl Mode {
    /// 방식 → 설정 문자열.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Fuzzy => "fuzzy",
            Self::Regex => "regex",
        }
    }

    /// 설정 문자열 → 방식(모르는 값 = 정확히).
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code.trim() {
            "fuzzy" => Self::Fuzzy,
            "regex" => Self::Regex,
            _ => Self::Exact,
        }
    }
}

/// 한 번 만들어 여러 항목에 쓰는 매처.
#[derive(Debug)]
pub struct Matcher {
    mode: Mode,
    /// 소문자 질의(정확히) 또는 단어들(유사).
    needle: String,
    words: Vec<String>,
    re: Option<Regex>,
    error: Option<String>,
}

impl Matcher {
    /// 질의가 비면 전부 통과한다.
    #[must_use]
    pub fn new(query: &str, mode: Mode) -> Self {
        let needle = query.trim().to_lowercase();
        let words: Vec<String> = needle.split_whitespace().map(str::to_string).collect();
        let (re, error) = if mode == Mode::Regex && !needle.is_empty() {
            match Regex::compile(&needle) {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
        };
        Self {
            mode,
            needle,
            words,
            re,
            error,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.needle.is_empty()
    }

    /// 정규식 컴파일 오류(있으면 정확히로 폴백해 동작 중).
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// `hay`(원문 대소문자 그대로)가 맞는가.
    #[must_use]
    pub fn matches(&self, hay: &str) -> bool {
        if self.needle.is_empty() {
            return true;
        }
        self.matches_lower(&hay.to_lowercase())
    }

    /// ★ 이미 소문자인 검색문(RAM 색인)용 — 키 입력마다 소문자 변환을 되풀이하지 않는다.
    #[must_use]
    pub fn matches_lower(&self, low: &str) -> bool {
        if self.needle.is_empty() {
            return true;
        }
        match (self.mode, &self.re) {
            (Mode::Regex, Some(re)) => re.is_match(low),
            (Mode::Fuzzy, _) => self.words.iter().all(|w| low.contains(w.as_str())),
            _ => low.contains(&self.needle),
        }
    }
}

/// ★ 검색문 만들기(09-04 색인) — 라벨 + 본문을 소문자로, `cap` 바이트(문자 경계) 상한.
#[must_use]
pub fn search_text(label: &str, body: Option<&str>, cap: usize) -> String {
    let mut t = label.to_lowercase();
    if let Some(b) = body {
        t.push('\n');
        t.push_str(&b.to_lowercase());
    }
    if t.len() > cap {
        let mut end = cap;
        while !t.is_char_boundary(end) {
            end -= 1;
        }
        t.truncate(end);
    }
    t
}

// ─────────────────────────────────────────────── 자체 정규식(백트래킹)

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class {
        neg: bool,
        items: Vec<ClassItem>,
    },
    Start,
    End,
    WordB,
    Group(Vec<Vec<Node>>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
    },
}

#[derive(Debug, Clone)]
enum ClassItem {
    Ch(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

/// 매칭 단계 상한 — 병적 패턴(`(a+)+b`)이 UI를 잡지 못하게.
const STEP_CAP: u32 = 200_000;

#[derive(Debug)]
struct Regex {
    alts: Vec<Vec<Node>>,
}

impl Regex {
    fn compile(pat: &str) -> Result<Self, String> {
        if pat.chars().count() > 256 {
            return Err("패턴이 너무 깁니다(256자)".into());
        }
        let chars: Vec<char> = pat.chars().collect();
        let mut p = Parser {
            s: &chars,
            i: 0,
            depth: 0,
        };
        let alts = p.parse_alts()?;
        if p.i < chars.len() {
            return Err(format!(
                "{}번째 글자 '{}'가 남았습니다(괄호 짝?)",
                p.i + 1,
                chars[p.i]
            ));
        }
        Ok(Self { alts })
    }

    fn is_match(&self, hay: &str) -> bool {
        let h: Vec<char> = hay.chars().collect();
        let steps = Cell::new(0u32);
        let anchored = self
            .alts
            .iter()
            .all(|a| matches!(a.first(), Some(Node::Start)));
        let ok = |start: usize| {
            self.alts
                .iter()
                .any(|alt| m_seq(alt, start, &h, &steps, &mut |_| true))
        };
        if anchored {
            return ok(0);
        }
        (0..=h.len()).any(|s| {
            if steps.get() >= STEP_CAP {
                return false;
            }
            ok(s)
        })
    }
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn parse_alts(&mut self) -> Result<Vec<Vec<Node>>, String> {
        let mut alts = vec![self.parse_seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.parse_seq()?);
        }
        Ok(alts)
    }

    fn parse_seq(&mut self) -> Result<Vec<Node>, String> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.parse_atom()?;
            let node = self.parse_quant(atom)?;
            seq.push(node);
        }
        Ok(seq)
    }

    fn parse_atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("패턴 끝")?;
        self.i += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                self.depth += 1;
                if self.depth > 16 {
                    return Err("괄호가 너무 깊습니다".into());
                }
                // (?:…) 는 그룹과 같다.
                if self.peek() == Some('?') && self.s.get(self.i + 1) == Some(&':') {
                    self.i += 2;
                }
                let alts = self.parse_alts()?;
                if self.peek() != Some(')') {
                    return Err("')'가 없습니다".into());
                }
                self.i += 1;
                self.depth -= 1;
                Node::Group(alts)
            }
            '[' => self.parse_class()?,
            '\\' => {
                let e = self.peek().ok_or("'\\' 뒤가 없습니다")?;
                self.i += 1;
                match e {
                    'd' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Digit(true)],
                    },
                    'D' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Digit(false)],
                    },
                    'w' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Word(true)],
                    },
                    'W' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Word(false)],
                    },
                    's' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Space(true)],
                    },
                    'S' => Node::Class {
                        neg: false,
                        items: vec![ClassItem::Space(false)],
                    },
                    'b' => Node::WordB,
                    'n' => Node::Char('\n'),
                    't' => Node::Char('\t'),
                    other => Node::Char(other),
                }
            }
            '*' | '+' | '?' => return Err(format!("'{c}' 앞에 대상이 없습니다")),
            other => Node::Char(other),
        })
    }

    fn parse_class(&mut self) -> Result<Node, String> {
        let mut neg = false;
        if self.peek() == Some('^') {
            neg = true;
            self.i += 1;
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or("']'가 없습니다")?;
            self.i += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = if c == '\\' {
                let e = self.peek().ok_or("'\\' 뒤가 없습니다")?;
                self.i += 1;
                match e {
                    'd' => {
                        items.push(ClassItem::Digit(true));
                        continue;
                    }
                    'w' => {
                        items.push(ClassItem::Word(true));
                        continue;
                    }
                    's' => {
                        items.push(ClassItem::Space(true));
                        continue;
                    }
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.s.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let hi = self.peek().ok_or("범위 끝이 없습니다")?;
                self.i += 1;
                if hi < lo {
                    return Err(format!("범위가 거꾸로입니다 [{lo}-{hi}]"));
                }
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Ch(lo));
            }
        }
        Ok(Node::Class { neg, items })
    }

    fn parse_quant(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let a = self.parse_num();
                let (min, max) = if self.peek() == Some(',') {
                    self.i += 1;
                    let b = self.parse_num();
                    (a, b)
                } else {
                    (a, a)
                };
                if self.peek() != Some('}') || a.is_none() {
                    // `{`를 리터럴로 본다(예: "{x}").
                    self.i = save;
                    return Ok(atom);
                }
                self.i += 1;
                if let (Some(lo), Some(hi)) = (min, max) {
                    if hi < lo {
                        return Err("{n,m}에서 m < n".into());
                    }
                }
                return self.finish_quant(atom, min.unwrap_or(0), max);
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        self.finish_quant(atom, min, max)
    }

    fn finish_quant(&mut self, atom: Node, min: u32, max: Option<u32>) -> Result<Node, String> {
        // 게으른 `?`는 탐욕과 같이 취급(부분 일치 판정엔 차이 없음).
        if self.peek() == Some('?') {
            self.i += 1;
        }
        if matches!(atom, Node::Start | Node::End | Node::WordB) {
            return Err("앵커에는 반복을 붙일 수 없습니다".into());
        }
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
        })
    }

    fn parse_num(&mut self) -> Option<u32> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if start == self.i {
            return None;
        }
        self.s[start..self.i]
            .iter()
            .collect::<String>()
            .parse::<u32>()
            .ok()
            .map(|n| n.min(1000))
    }
}

fn class_hit(neg: bool, items: &[ClassItem], c: char) -> bool {
    let hit = items.iter().any(|it| match it {
        ClassItem::Ch(x) => *x == c,
        ClassItem::Range(a, b) => (*a..=*b).contains(&c),
        ClassItem::Digit(pos) => c.is_ascii_digit() == *pos,
        ClassItem::Word(pos) => (c.is_alphanumeric() || c == '_') == *pos,
        ClassItem::Space(pos) => c.is_whitespace() == *pos,
    });
    hit != neg
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn m_seq(
    seq: &[Node],
    pos: usize,
    h: &[char],
    steps: &Cell<u32>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let n = steps.get() + 1;
    steps.set(n);
    if n > STEP_CAP {
        return false;
    }
    match seq.split_first() {
        None => k(pos),
        Some((node, rest)) => m_node(node, pos, h, steps, &mut |p| m_seq(rest, p, h, steps, k)),
    }
}

fn m_node(
    node: &Node,
    pos: usize,
    h: &[char],
    steps: &Cell<u32>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    match node {
        Node::Char(c) => pos < h.len() && h[pos] == *c && k(pos + 1),
        Node::Any => pos < h.len() && h[pos] != '\n' && k(pos + 1),
        Node::Class { neg, items } => pos < h.len() && class_hit(*neg, items, h[pos]) && k(pos + 1),
        Node::Start => pos == 0 && k(pos),
        Node::End => pos == h.len() && k(pos),
        Node::WordB => {
            let before = pos > 0 && is_word(h[pos - 1]);
            let after = pos < h.len() && is_word(h[pos]);
            before != after && k(pos)
        }
        Node::Group(alts) => alts.iter().any(|alt| m_seq(alt, pos, h, steps, k)),
        Node::Repeat { node, min, max } => m_rep(node, *min, *max, 0, pos, h, steps, k),
    }
}

#[allow(clippy::too_many_arguments)]
fn m_rep(
    node: &Node,
    min: u32,
    max: Option<u32>,
    count: u32,
    pos: usize,
    h: &[char],
    steps: &Cell<u32>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    if steps.get() > STEP_CAP {
        return false;
    }
    // 탐욕: 하나 더 먹어 본다(빈 일치로 제자리면 중단) → 안 되면 여기서 멈춘다.
    if max.is_none_or(|m| count < m)
        && m_node(node, pos, h, steps, &mut |p| {
            p != pos && m_rep(node, min, max, count + 1, p, h, steps, k)
        })
    {
        return true;
    }
    count >= min && k(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_fuzzy_search_whole_text() {
        let m = Matcher::new("nexa-app", Mode::Exact);
        assert!(m.matches(
            "-08-24 19:51 700 libnexa_vfs.d\nla--- 2026-09-04 16:05 3889152 nexa-app.exe"
        ));
        assert!(!m.matches("libnexa_vfs.rlib"));
        assert!(
            Matcher::new("NEXA App", Mode::Fuzzy).matches("nexa-app.exe"),
            "단어 전부 · 대소문자 무시"
        );
        assert!(!Matcher::new("nexa zzz", Mode::Fuzzy).matches("nexa-app.exe"));
        assert!(Matcher::new("   ", Mode::Regex).is_empty());
    }

    #[test]
    fn regex_basics() {
        let ok = |p: &str, h: &str| Matcher::new(p, Mode::Regex).matches(h);
        assert!(ok("nexa[-_]app", "x nexa_app.pdb"));
        assert!(ok(r"\d{4}-\d{2}-\d{2}", "la--- 2026-09-04 16:05"));
        assert!(!ok(r"^\d{4}", "la--- 2026"));
        assert!(ok(r"^la---", "la--- 2026"));
        assert!(ok(r"\.exe$", "nexa-app.exe"));
        assert!(ok("a.c", "abc") && !ok("a.c", "ac"));
        assert!(ok("colou?r", "color") && ok("colou?r", "colour"));
        assert!(ok("(png|jpg)$", "shot.png") && !ok("(png|jpg)$", "shot.gif"));
        assert!(ok(r"\bapp\b", "nexa app x") && !ok(r"\bapp\b", "nexa-application"));
        assert!(ok("[^a-z]", "abc1") && !ok("[^a-z]", "abc"));
        assert!(ok("x{2,3}", "axxb") && !ok("x{2,3}", "axb"));
        assert!(ok(r"\s\w+", "a b"));
        assert!(ok("a{x}", "a{x}"), "{{ 리터럴 폴백");
        assert!(ok("한글.*검색", "한글 정규식 검색"));
    }

    #[test]
    fn regex_errors_fall_back_and_never_hang() {
        let m = Matcher::new("(abc", Mode::Regex);
        assert!(m.error().is_some());
        assert!(m.matches("x(abc y"), "폴백 = 정확히(리터럴)");
        assert!(Matcher::new("*a", Mode::Regex).error().is_some());
        // 병적 패턴 — 단계 상한으로 끝난다.
        let bad = Matcher::new("(a+)+$", Mode::Regex);
        let hay = "a".repeat(40) + "b";
        assert!(!bad.matches(&hay));
    }
}
