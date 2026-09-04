//! ★ T-18d 1단 — 제한 리치텍스트 **런 파서**(09-03 사용자 요청 "CopyQ처럼 색 유지").
//!
//! CopyQ는 `text/html` 표현을 Qt(QTextDocument)에 위임해 그린다 — 구문강조는
//! **원본 편집기가 이미 HTML에 입혀 놓은 것**이다. 우리는 같은 재료(CF_HTML,
//! 캡처 때 정제됨)를 **색·굵기 런**으로 풀고, 자체 래스터라이저가 그린다(DR-8).
//!
//! ## 범위(1단)
//! - 인라인: `<span style>`·`<font color>`·`<b>/<strong>`·`<i>/<em>` — 색·굵기·기울임
//! - 블록: `<br>`·`<div>`·`<p>`·`<li>`·`<tr>` = 줄 바꿈. `<style>`·`<script>` 내용 스킵
//! - 엔티티: 기본 5종 + `&nbsp;` + 수치(`&#..;`·`&#x..;`)
//!
//! ## 2단(09-04 — 새 Outlook 메일 복사 실기 "n ■" · CopyQ 비교)
//! - ★ **심볼 글꼴 치환**: `font-family: Wingdings/Symbol` 런의 글자를 유니코드 등가로(Word·Outlook 불릿은
//!   "심볼 글꼴 + 일반 문자"라 글꼴을 버리면 `n`이 보인다 — 평문 표현도 `n`이므로 이 치환은 리치 경로에서만).
//! - **목록**: `<ul>/<ol>` 깊이별 불릿(• ◦ ▪ · `1.`) 합성 + `<li>` 들여쓰기.
//! - **들여쓰기**: 블록의 `margin-left`·`padding-left`·`text-indent`(pt·px·cm·in·em)를 em으로 — 줄의 첫 런에 실린다.
//! - **글자 배율**: `font-size` 상대값(본문 10pt 기준 ±15% 안은 1.0 · 0.8~1.3으로 묶음).
//! - `<img>`: `data:image/…;base64` 원본 바이트를 [`Run::image`]로(Outlook은 표를 이렇게 넣는다 — 09-04 사용자
//!   "붙여넣기는 이미지가 나오니 미리보기도"). 텍스트는 `[image]`(행·폴백). 디코드는 앱(격리 워커) 몫.
//! - 표 구조·배경색은 아직 밖.

/// 스타일 런 — 같은 스타일이 이어지는 텍스트 조각.
#[derive(Clone, PartialEq, Debug)]
pub struct Run {
    /// 조각 텍스트(엔티티 해제 후).
    pub text: String,
    /// 글자색(RGB) — 없으면 테마 본문색.
    pub color: Option<[u8; 3]>,
    /// 굵게.
    pub bold: bool,
    /// 기울임.
    pub italic: bool,
    /// ★ 2단: 줄 들여쓰기(em) — 줄의 **첫 런**에만 실린다(목록 · `margin-left`). 그리는 쪽은
    /// `x += em × indent` 한 줄이면 된다([`em_px`]).
    pub indent: f32,
    /// ★ 2단: 글자 배율(1.0 = 본문) — 0.8~1.3. 줄 간격은 고정이라 이 범위를 넘기지 않는다.
    pub scale: f32,
    /// ★ 인라인 이미지 원본 바이트(PNG/JPEG — `data:` URI에서 풀어 둔 것). 그리는 쪽이 디코드해 그리고,
    /// 못 그리면 `text`(`[image]`)를 쓴다. `Arc` = 행·미리보기가 런을 복제해도 바이트는 한 벌.
    pub image: Option<std::sync::Arc<Vec<u8>>>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: None,
            bold: false,
            italic: false,
            indent: 0.0,
            scale: 1.0,
            image: None,
        }
    }
}

/// em 단위 → px(반올림). 그리기 쪽이 들여쓰기에 쓴다 — `em`은 본문 글꼴의 전각 폭.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn em_px(em: i32, factor: f32) -> i32 {
    (em as f32 * factor).round() as i32
}

/// 배율 → 글꼴 크기 증분(px) — `select_font_sized`의 delta.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn size_delta(em: i32, scale: f32) -> f32 {
    em as f32 * (scale - 1.0)
}

/// 심볼 글꼴 — 글자 코드가 곧 그림인 글꼴(Word·Outlook 불릿).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Sym {
    #[default]
    None,
    Wingdings,
    Symbol,
    /// 그 밖의 심볼 글꼴(Webdings·Wingdings 2/3) — 표는 없고 PUA만 벗긴다.
    Other,
}

#[derive(Clone, Copy, Default)]
struct Style {
    color: Option<[u8; 3]>,
    bold: bool,
    italic: bool,
    /// ★ 공백 보존(`<pre>`·`white-space: pre*` · 09-03 실기 — Sublime계 HTML은
    /// 블록 태그 없이 **원시 개행**으로 줄을 나눈다): 켜지면 개행 = 줄 바꿈.
    pre: bool,
    /// ★ 2단: 심볼 글꼴 — 텍스트 노드의 글자를 치환한다.
    sym: Sym,
    /// ★ 2단: 인라인 `font-size` 배율 — 0.0 = 미지정(블록 값을 따른다).
    scale: f32,
}

/// 블록(문단·목록·항목) — 들여쓰기·배율·목록 번호를 쌓는다.
struct Block {
    name: String,
    /// 이 블록이 더하는 들여쓰기(em).
    indent: f32,
    /// 블록 글자 배율(부모 상속).
    scale: f32,
    /// `<ol>`이면 번호 카운터.
    ordered: Option<u32>,
}

/// 표현 목록에서 HTML을 찾아 줄×런으로 푼다 — 없거나 못 풀면 `None`(평문 폴백).
///
/// `max_lines` 줄에서 끊는다(목록 행은 몇 줄만 필요 — 거대 문서 방어).
#[must_use]
pub fn html_runs_of(reps: &[crate::RawRep], max_lines: usize) -> Option<Vec<Vec<Run>>> {
    let r = reps
        .iter()
        .find(|r| matches!(r.format.as_str(), "CF_HTML" | "HTML Format" | "text/html"))?;
    let html = core::str::from_utf8(&r.data).ok()?;
    let (runs, structured) = parse(fragment_of(html), max_lines);
    // 색·서식이 하나도 없으면 평문과 같다 — 굳이 리치 경로를 태우지 않는다.
    // ★ 2단: 목록 불릿·들여쓰기·심볼 치환·이미지 자리표시는 평문이 잃은 구조라 리치로 친다.
    let styled = structured
        || runs.iter().flatten().any(|run| {
            run.color.is_some() || run.bold || run.italic || (run.scale - 1.0).abs() > f32::EPSILON
        });
    (styled && !runs.is_empty()).then_some(runs)
}

/// CF_HTML 헤더를 벗기고 조각만 — `StartFragment` 주석이 정본, 없으면 전체.
fn fragment_of(html: &str) -> &str {
    let start = html.find("<!--StartFragment-->").map_or_else(
        || html.find('<').unwrap_or(0),
        |i| i + "<!--StartFragment-->".len(),
    );
    let end = html.find("<!--EndFragment-->").unwrap_or(html.len());
    html.get(start..end).unwrap_or(html)
}

/// 본체 — 태그 스택 워커(파싱 실패 지점은 그냥 지나친다: 클립보드는 남의 데이터다).
///
/// 반환 = (줄×런, 구조 있음) — 구조 = 불릿·들여쓰기·심볼 치환·이미지 자리표시 중 하나라도.
fn parse(frag: &str, max_lines: usize) -> (Vec<Vec<Run>>, bool) {
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    let mut stack: Vec<Style> = Vec::new();
    let mut cur = Style::default();
    let mut blocks: Vec<Block> = Vec::new();
    // 구조 사건 수(불릿·들여쓰기·심볼 치환·이미지) — 0이면 평문과 같다.
    let mut marks = 0u32;
    let mut text = String::new();
    let bytes = frag.as_bytes();
    let mut i = 0usize;
    // 직전이 공백이었나 — HTML 공백 붕괴(연속 공백·개행 = 한 칸). &nbsp;·탭은 보존.
    let mut last_ws = true;

    macro_rules! flush {
        () => {
            if !text.is_empty() {
                let line = lines.last_mut().unwrap_or_else(|| unreachable!());
                let indent = if line.is_empty() {
                    block_indent(&blocks)
                } else {
                    0.0
                };
                if indent > 0.0 {
                    marks += 1;
                }
                let scale = if cur.scale > 0.0 {
                    cur.scale
                } else {
                    blocks.last().map_or(1.0, |b| b.scale)
                };
                line.push(Run {
                    text: core::mem::take(&mut text),
                    color: cur.color,
                    bold: cur.bold,
                    italic: cur.italic,
                    indent,
                    scale,
                    image: None,
                });
            }
        };
    }
    macro_rules! new_line {
        () => {
            if !lines.last().is_none_or(Vec::is_empty) {
                lines.push(Vec::new());
            }
        };
    }

    while i < bytes.len() && lines.len() <= max_lines {
        if bytes[i] == b'<' {
            let Some(close) = frag[i..].find('>') else {
                break;
            };
            let tag = &frag[i + 1..i + close];
            i += close + 1;
            let (closing, name) = tag_name(tag);
            match (closing, name.as_str()) {
                // 내용 스킵 블록 — 정제가 못 지운 <style> CSS 본문이 글로 새지 않게.
                (false, "style" | "script") => {
                    let end_pat = if name == "style" {
                        "</style"
                    } else {
                        "</script"
                    };
                    if let Some(e) = frag[i..].to_ascii_lowercase().find(end_pat) {
                        i += e;
                    } else {
                        break;
                    }
                }
                (false, "br") => {
                    flush!();
                    lines.push(Vec::new());
                    last_ws = true;
                }
                // ★ 이미지 — `data:` URI면 원본 바이트를 런에 싣는다(09-04 · Outlook 표). 아니면 `[image]` 자리표시.
                (false, "img") => {
                    flush!();
                    let low = tag.to_ascii_lowercase();
                    let image = attr_value(&low, tag, "src")
                        .and_then(data_image_bytes)
                        .map(std::sync::Arc::new);
                    text.push_str("[image]");
                    flush!();
                    if let Some(run) = lines.last_mut().and_then(|l| l.last_mut()) {
                        run.image = image;
                    }
                    marks += 1;
                    last_ws = true;
                }
                // 표 행 = 줄 바꿈(표 구조는 아직 밖).
                (_, "tr") => {
                    flush!();
                    new_line!();
                    last_ws = true;
                }
                // ★ 2단: 블록 = 줄 바꿈 + 들여쓰기·배율 스택. 목록(ul/ol)은 줄을 바꾸지 않고 깊이만 더한다.
                (false, "div" | "p" | "li" | "h1" | "h2" | "h3" | "ul" | "ol") => {
                    flush!();
                    let is_list = matches!(name.as_str(), "ul" | "ol");
                    if !is_list {
                        new_line!();
                    }
                    let (indent, scale) = block_attrs(tag);
                    let parent_scale = blocks.last().map_or(1.0, |b| b.scale);
                    blocks.push(Block {
                        name: name.clone(),
                        // 목록 기본 들여쓰기 1.5em(브라우저 40px에 해당 — 행 폭이 좁아 조금 줄인다).
                        indent: indent.unwrap_or(if is_list { 1.5 } else { 0.0 }),
                        scale: scale.unwrap_or(parent_scale),
                        ordered: (name == "ol").then_some(0),
                    });
                    if name == "li" {
                        marks += 1;
                        text.push_str(&list_marker(&mut blocks));
                    }
                    last_ws = true;
                }
                (true, "div" | "p" | "li" | "h1" | "h2" | "h3" | "ul" | "ol") => {
                    flush!();
                    if let Some(pos) = blocks.iter().rposition(|b| b.name == name) {
                        blocks.truncate(pos);
                    }
                    if !matches!(name.as_str(), "ul" | "ol") {
                        new_line!();
                    }
                    last_ws = true;
                }
                // ★ pre = 공백 보존 구간(09-03) — 스타일 스택으로 복원된다.
                (false, "pre") => {
                    flush!();
                    new_line!();
                    stack.push(cur);
                    cur.pre = true;
                    last_ws = true;
                }
                (true, "pre") => {
                    flush!();
                    if let Some(prev) = stack.pop() {
                        cur = prev;
                    }
                    new_line!();
                    last_ws = true;
                }
                (false, "b" | "strong") => {
                    flush!();
                    stack.push(cur);
                    cur.bold = true;
                }
                (false, "i" | "em") => {
                    flush!();
                    stack.push(cur);
                    cur.italic = true;
                }
                (false, "span" | "font") => {
                    flush!();
                    stack.push(cur);
                    apply_attrs(tag, &mut cur);
                }
                (true, "b" | "strong" | "i" | "em" | "span" | "font") => {
                    flush!();
                    if let Some(prev) = stack.pop() {
                        cur = prev;
                    }
                }
                _ => {}
            }
        } else {
            // 텍스트 노드 — 엔티티 해제 + 공백 붕괴(개행 포함 · 탭은 보존).
            let next = frag[i..].find('<').map_or(frag.len(), |n| i + n);
            for ch in decode_entities(&frag[i..next]).chars() {
                match ch {
                    '\r' => {}
                    // ★ pre 구간: 개행 = 줄 바꿈 · 공백 그대로(09-03 — Sublime계 HTML).
                    '\n' if cur.pre => {
                        flush!();
                        lines.push(Vec::new());
                        last_ws = true;
                    }
                    ' ' if cur.pre => {
                        text.push(' ');
                        last_ws = false;
                    }
                    ' ' | '\n' => {
                        if !last_ws {
                            text.push(' ');
                        }
                        last_ws = true;
                    }
                    '\u{a0}' => {
                        // &nbsp; = 보존 공백(편집기 들여쓰기) — 붕괴하지 않는다.
                        text.push(' ');
                        last_ws = false;
                    }
                    c => {
                        // ★ 2단: 심볼 글꼴 치환 — Wingdings 'n' → ■ (Outlook 불릿).
                        let m = sym_map(cur.sym, c);
                        if m != c {
                            marks += 1;
                        }
                        text.push(m);
                        last_ws = m == '\t';
                    }
                }
            }
            i = next;
        }
    }
    flush!();
    while lines.last().is_some_and(Vec::is_empty) && lines.len() > 1 {
        lines.pop();
    }
    lines.truncate(max_lines);
    (lines, marks > 0)
}

/// `data:image/…;base64,…` → 원본 바이트. 이미지 MIME + base64만 · 8MiB 상한(클립보드는 남의 데이터).
fn data_image_bytes(src: &str) -> Option<Vec<u8>> {
    let rest = src.trim().strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    let mut parts = meta.split(';');
    if !parts.next()?.trim().starts_with("image/") || !parts.any(|p| p.trim() == "base64") {
        return None;
    }
    if payload.len() > 11 << 20 {
        return None;
    }
    let bytes = base64_decode(payload)?;
    (!bytes.is_empty()).then_some(bytes)
}

/// 표준 base64(+ URL-safe 두 글자) 해제 — 공백·개행 무시 · 패딩 관대. 외부 crate 없이(DR-8).
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => u32::from(b - b'A'),
            b'a'..=b'z' => u32::from(b - b'a') + 26,
            b'0'..=b'9' => u32::from(b - b'0') + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// 활성 블록의 들여쓰기 합(em) — 0~12로 묶는다(행 폭 방어).
fn block_indent(blocks: &[Block]) -> f32 {
    blocks
        .iter()
        .map(|b| b.indent)
        .sum::<f32>()
        .clamp(0.0, 12.0)
}

/// `<li>` 불릿 — 가장 안쪽 목록이 `<ol>`이면 번호, 아니면 깊이별 • ◦ ▪.
fn list_marker(blocks: &mut [Block]) -> String {
    let depth = blocks
        .iter()
        .filter(|b| matches!(b.name.as_str(), "ul" | "ol"))
        .count();
    if let Some(list) = blocks
        .iter_mut()
        .rev()
        .find(|b| matches!(b.name.as_str(), "ul" | "ol"))
    {
        if let Some(n) = list.ordered.as_mut() {
            *n += 1;
            return format!("{n}. ");
        }
    }
    match depth {
        0 | 1 => "• ",
        2 => "◦ ",
        _ => "▪ ",
    }
    .to_string()
}

/// 심볼 글꼴 글자 → 유니코드 등가. Word/Outlook 불릿 라이브러리(■ ● ◆ ❖ ➢ ✓ …)를 덮는다 —
/// 표에 없는 글자는 그대로. PUA(`U+F0xx` — Word가 심볼 글꼴 글자를 이렇게 내보내기도 한다)는 먼저 벗긴다.
fn sym_map(sym: Sym, c: char) -> char {
    let (sym, c) = match c {
        '\u{F020}'..='\u{F0FF}' => (
            if sym == Sym::None {
                Sym::Wingdings
            } else {
                sym
            },
            char::from_u32(c as u32 - 0xF000).unwrap_or(c),
        ),
        _ => (sym, c),
    };
    match sym {
        Sym::Wingdings => match c {
            'l' => '●',
            'm' => '❍',
            'n' => '■',
            'o' => '□',
            'p' => '❑',
            'q' => '❒',
            'u' => '◆',
            'v' => '❖',
            '§' => '▪',
            'Ø' => '➢',
            'ü' => '✓',
            'ý' => '✔',
            'û' => '✗',
            'þ' => '☑',
            'è' => '➔',
            'ð' => '⇨',
            'J' => '☺',
            'L' => '☹',
            _ => c,
        },
        Sym::Symbol => match c {
            '·' => '•',
            _ => c,
        },
        Sym::None | Sym::Other => c,
    }
}

/// `font-family` 값 → 심볼 글꼴 분류.
fn sym_of(v: &str) -> Sym {
    // 속성값은 엔티티 해제 전 — Outlook은 글꼴 이름을 `&quot;…&quot;`로 감싼다.
    let low = v.to_ascii_lowercase().replace("&quot;", "\"");
    let first = low
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();
    match first {
        "wingdings" => Sym::Wingdings,
        "symbol" => Sym::Symbol,
        _ if first.starts_with("wingdings") || first == "webdings" => Sym::Other,
        _ => Sym::None,
    }
}

/// CSS 길이 → em(본문 10pt = 13.33px 기준). 단위 없는 0만 허용.
fn len_em(v: &str) -> Option<f32> {
    let v = v.trim();
    let split = v
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(v.len());
    let n: f32 = v[..split].trim().parse().ok()?;
    let unit = v[split..].trim().to_ascii_lowercase();
    Some(match unit.as_str() {
        "pt" => n / 10.0,
        "px" => n / 13.333,
        "cm" => n * 2.835,
        "mm" => n * 0.2835,
        "in" => n * 7.2,
        "em" | "rem" => n,
        "" if n.abs() < f32::EPSILON => 0.0,
        _ => return None,
    })
}

/// `font-size` → 배율. 본문(10pt·13.33px) ±15%는 1.0 — Outlook은 15px/10pt를 섞어 쓴다. 0.8~1.3.
fn scale_of(v: &str) -> Option<f32> {
    let v = v.trim();
    let s = if let Some(p) = v.strip_suffix('%') {
        p.trim().parse::<f32>().ok()? / 100.0
    } else {
        len_em(v)?
    };
    if !(0.05..10.0).contains(&s) {
        return None;
    }
    Some(if (s - 1.0).abs() < 0.15 {
        1.0
    } else {
        s.clamp(0.8, 1.3)
    })
}

/// 블록 태그의 `style`에서 (들여쓰기 em, 배율) — 들여쓰기 = margin-left(단축형 포함) + padding-left + text-indent.
fn block_attrs(tag: &str) -> (Option<f32>, Option<f32>) {
    let low = tag.to_ascii_lowercase();
    let Some(v) = attr_value(&low, tag, "style") else {
        return (None, None);
    };
    let (mut indent, mut has, mut scale) = (0.0f32, false, None);
    for decl in v.split(';') {
        let Some((k, val)) = decl.split_once(':') else {
            continue;
        };
        let (k, val) = (k.trim().to_ascii_lowercase(), val.trim());
        match k.as_str() {
            "margin-left" | "padding-left" | "text-indent" => {
                if let Some(e) = len_em(val) {
                    indent += e;
                    has = true;
                }
            }
            "margin" | "padding" => {
                let parts: Vec<&str> = val.split_whitespace().collect();
                let left = match parts.len() {
                    1 => parts[0],
                    2 | 3 => parts[1],
                    4 => parts[3],
                    _ => continue,
                };
                if let Some(e) = len_em(left) {
                    indent += e;
                    has = true;
                }
            }
            "font-size" => scale = scale_of(val),
            _ => {}
        }
    }
    (has.then_some(indent.max(0.0)), scale)
}

/// 태그 이름(소문자) + 닫는 태그 여부.
fn tag_name(tag: &str) -> (bool, String) {
    let t = tag.trim();
    let (closing, t) = t.strip_prefix('/').map_or((false, t), |r| (true, r));
    let name: String = t
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    (closing, name)
}

/// `style="…"`·`color="…"` 속성에서 색·굵기·기울임을 뽑는다.
fn apply_attrs(tag: &str, st: &mut Style) {
    let low = tag.to_ascii_lowercase();
    if let Some(v) = attr_value(&low, tag, "style") {
        for decl in v.split(';') {
            let Some((k, val)) = decl.split_once(':') else {
                continue;
            };
            let (k, val) = (k.trim().to_ascii_lowercase(), val.trim());
            match k.as_str() {
                "color" => {
                    if let Some(c) = parse_color(val) {
                        st.color = Some(c);
                    }
                }
                "font-weight" => {
                    let n: u32 = val.parse().unwrap_or(0);
                    if val.eq_ignore_ascii_case("bold") || n >= 600 {
                        st.bold = true;
                    } else if val.eq_ignore_ascii_case("normal") || (1..600).contains(&n) {
                        st.bold = false;
                    }
                }
                "white-space" => {
                    let v = val.to_ascii_lowercase();
                    if v.starts_with("pre") {
                        st.pre = true;
                    } else if v == "normal" || v == "nowrap" {
                        st.pre = false;
                    }
                }
                "font-style" => {
                    if val.eq_ignore_ascii_case("italic") {
                        st.italic = true;
                    } else if val.eq_ignore_ascii_case("normal") {
                        st.italic = false;
                    }
                }
                // ★ 2단: 심볼 글꼴 · 인라인 글자 배율.
                "font-family" => st.sym = sym_of(val),
                "font-size" => {
                    if let Some(sc) = scale_of(val) {
                        st.scale = sc;
                    }
                }
                _ => {}
            }
        }
    }
    // <font color="#..."> — 레거시(Word 등).
    if let Some(v) = attr_value(&low, tag, "color") {
        if let Some(c) = parse_color(v.trim()) {
            st.color = Some(c);
        }
    }
}

/// 소문자 사본에서 위치를 찾아 **원본**에서 따옴표 값을 뽑는다(값의 대소문자 보존).
fn attr_value<'a>(low: &str, orig: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=");
    let mut from = 0usize;
    loop {
        let k = low[from..].find(&pat)? + from;
        // 속성 이름 경계 확인 — bgcolor= 가 color= 에 걸리지 않게.
        if k > 0 && low.as_bytes()[k - 1].is_ascii_alphanumeric() {
            from = k + pat.len();
            continue;
        }
        let rest = &orig[k + pat.len()..];
        let mut chars = rest.chars();
        return match chars.next() {
            Some(q @ ('"' | '\'')) => {
                let body = &rest[1..];
                body.find(q).map(|e| &body[..e])
            }
            Some(_) => Some(
                rest.split(|c: char| c.is_whitespace() || c == '>')
                    .next()
                    .unwrap_or(""),
            ),
            None => None,
        };
    }
}

/// `#rrggbb` · `#rgb` · `rgb(r, g, b)` — 그 외(이름 색 등)는 1단 밖.
fn parse_color(v: &str) -> Option<[u8; 3]> {
    if let Some(hex) = v.strip_prefix('#') {
        return match hex.len() {
            6 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
            }
            3 => {
                let n = u32::from_str_radix(hex, 16).ok()?;
                let (r, g, b) = ((n >> 8) & 0xF, (n >> 4) & 0xF, n & 0xF);
                Some([(r * 17) as u8, (g * 17) as u8, (b * 17) as u8])
            }
            _ => None,
        };
    }
    let inner = v
        .strip_prefix("rgb(")
        .or_else(|| v.strip_prefix("RGB("))?
        .strip_suffix(')')?;
    let mut it = inner.split(',').map(|p| p.trim().parse::<u8>());
    match (it.next(), it.next(), it.next()) {
        (Some(Ok(r)), Some(Ok(g)), Some(Ok(b))) => Some([r, g, b]),
        _ => None,
    }
}

/// 기본 엔티티 + 수치 참조.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        // ★ 12바이트 창을 **문자 경계**로 내린다(09-04 실기 — `&` 뒤에 한글이 오면 12번째 바이트가
        //   글자 안이라 슬라이스가 패닉했다 · 메인창 리치 행 생성 중 프로세스 종료).
        let lim = (0..=rest.len().min(12))
            .rev()
            .find(|&n| rest.is_char_boundary(n))
            .unwrap_or(0);
        let Some(semi) = rest[..lim].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let decoded = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => ent
                .strip_prefix('#')
                .and_then(|n| {
                    n.strip_prefix('x')
                        .or_else(|| n.strip_prefix('X'))
                        .map_or_else(
                            || n.parse::<u32>().ok(),
                            |h| u32::from_str_radix(h, 16).ok(),
                        )
                })
                .and_then(char::from_u32),
        };
        if let Some(c) = decoded {
            out.push(c);
            rest = &rest[semi + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ 09-04 실기: `&` 뒤 12바이트 안에 다중바이트 글자가 걸려도 패닉하지 않는다.
    #[test]
    fn entity_window_respects_char_boundary() {
        assert_eq!(
            decode_entities("a&b 한글 아이콘 텍스트"),
            "a&b 한글 아이콘 텍스트"
        );
        assert_eq!(decode_entities("&amp;콘"), "&콘");
        assert_eq!(decode_entities("&한글한글한글;"), "&한글한글한글;");
    }

    fn rep(fmt: &str, html: &str) -> crate::RawRep {
        crate::RawRep {
            format: fmt.into(),
            data: html.as_bytes().to_vec(),
        }
    }

    /// VSCode류 조각 — div 줄 + span 색 + &nbsp; 들여쓰기.
    #[test]
    fn editor_fragment_colors_and_lines() {
        let html = "Version:0.9\r\n<!--StartFragment--><div>\
            <span style=\"color: #cd3131;\">SELECT</span></div>\
            <div><span>&nbsp;&nbsp;</span><span style=\"color:#001080\">A.BOM_ITEM_CD</span></div>\
            <!--EndFragment-->";
        let runs = html_runs_of(&[rep("CF_HTML", html)], 6).expect("리치 런");
        assert_eq!(runs.len(), 2, "div 2개 = 2줄");
        assert_eq!(runs[0][0].text, "SELECT");
        assert_eq!(runs[0][0].color, Some([0xCD, 0x31, 0x31]));
        // &nbsp; 들여쓰기 보존.
        assert!(runs[1][0].text.starts_with("  "), "{:?}", runs[1]);
        assert_eq!(runs[1].last().unwrap().color, Some([0x00, 0x10, 0x80]));
    }

    /// b/i·font color·수치 엔티티 — 스택 복원까지.
    #[test]
    fn inline_styles_nest_and_pop() {
        let html = "<b>bold <i>bi</i></b> <font color=\"#0f0\">g</font> plain &#65;";
        let runs = html_runs_of(&[rep("text/html", html)], 6).expect("리치 런");
        let line = &runs[0];
        assert_eq!(
            (line[0].text.as_str(), line[0].bold, line[0].italic),
            ("bold ", true, false)
        );
        assert_eq!((line[1].text.as_str(), line[1].italic), ("bi", true));
        assert_eq!(line[2].color, None, "닫힌 뒤 색 복원: {line:?}");
        let g = line.iter().find(|r| r.text == "g").expect("font 색 런");
        assert_eq!(g.color, Some([0, 255, 0]));
        assert!(line.last().unwrap().text.contains('A'), "&#65; = A");
    }

    /// 서식이 전혀 없으면 None — 평문 경로가 이긴다.
    #[test]
    fn plain_html_is_none() {
        assert!(html_runs_of(&[rep("CF_HTML", "<div>hi</div>")], 6).is_none());
        assert!(html_runs_of(&[rep("CF_UNICODETEXT", "x")], 6).is_none());
    }

    /// ★ pre 구간 — 원시 개행 = 줄 바꿈 · 공백 보존(Sublime계 · 09-03 실기).
    #[test]
    fn pre_newlines_become_lines() {
        let html = "<pre><span style=\"color:#ff0000\">SELECT</span>\n    *\nFROM t</pre>";
        let runs = html_runs_of(&[rep("CF_HTML", html)], 9).expect("리치 런");
        assert_eq!(runs.len(), 3, "{runs:?}");
        assert_eq!(runs[1][0].text, "    *", "들여쓰기 보존: {runs:?}");
        let html2 = "<span style=\"white-space: pre-wrap; color:#111\">a\nb</span>";
        let runs2 = html_runs_of(&[rep("text/html", html2)], 9).expect("리치 런");
        assert_eq!(runs2.len(), 2, "{runs2:?}");
    }

    /// ★ 2단(09-04 실기 — 새 Outlook 메일 복사, 구조만 옮긴 축약본): Wingdings 'n' = ■ ·
    /// `<ul><li>` 불릿 합성 + 들여쓰기 · `margin-left: 58pt` 문단 · `data:` 이미지 자리표시 ·
    /// 15px/10pt는 본문 배율 1.0.
    #[test]
    fn outlook_list_bullets_and_indent() {
        let html = concat!(
            "<p style=\"margin: 0cm 0cm 0.0001pt 40pt; font-size: 10pt; text-indent: -20pt\">",
            "<span style=\"font-family: Wingdings\">n<span style=\"font: 7pt Times\">&nbsp;&nbsp;</span></span>",
            "<b><span style=\"font-family: &quot;A&quot;\">S&amp;OP</span></b></p>",
            "<ul style=\"font-size: 15px\"><li style=\"margin: 0cm 0cm 0.0001pt 22pt; font-size: 10pt\">일시 : 9/8</li>",
            "<li style=\"margin-left: 22pt\">장소</li></ul>",
            "<p style=\"margin-left: 58pt\">* 참고</p>",
            "<p><img src=\"data:image/png;base64,AAAA\"></p>",
        );
        let runs = html_runs_of(&[rep("HTML Format", html)], 20).expect("리치 런");
        let joined: Vec<String> = runs
            .iter()
            .map(|l| l.iter().map(|r| r.text.as_str()).collect())
            .collect();
        assert_eq!(joined.len(), 5, "{joined:?}");
        assert!(joined[0].starts_with("■  S&OP"), "{joined:?}");
        assert!(
            (runs[0][0].indent - 2.0).abs() < 0.01,
            "40pt−20pt = 2em: {:?}",
            runs[0][0]
        );
        assert!(runs[0].iter().any(|r| r.bold && r.text == "S&OP"));
        assert!(joined[1].starts_with("• 일시"), "{joined:?}");
        assert!(
            (runs[1][0].indent - 3.7).abs() < 0.01,
            "ul 1.5 + li 2.2: {:?}",
            runs[1][0]
        );
        assert!(
            (runs[1][0].scale - 1.0).abs() < f32::EPSILON,
            "15px/10pt = 본문"
        );
        assert!(joined[2].starts_with("• 장소"), "{joined:?}");
        assert!((runs[3][0].indent - 5.8).abs() < 0.01, "{:?}", runs[3][0]);
        assert_eq!(joined[4], "[image]");
        assert_eq!(
            runs[4][0].image.as_deref().map(Vec::as_slice),
            Some(&[0u8, 0, 0][..]),
            "data: 바이트가 런에 실린다"
        );
        // 두 번째 줄부터의 런은 들여쓰기를 다시 싣지 않는다.
        assert!(runs[0]
            .iter()
            .skip(1)
            .all(|r| r.indent.abs() < f32::EPSILON));
    }

    /// base64·data: URI — PNG 시그니처 왕복 · 비이미지/비base64는 None · 잡문자는 None.
    #[test]
    fn data_uri_and_base64() {
        assert_eq!(
            base64_decode("iVBORw0KGgo=").as_deref(),
            Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..])
        );
        assert_eq!(
            base64_decode("aGVs\nbG8").as_deref(),
            Some(&b"hello"[..]),
            "패딩 없음·개행"
        );
        assert!(base64_decode("a*b").is_none());
        assert!(data_image_bytes("data:image/png;base64,iVBORw0KGgo=").is_some());
        assert!(data_image_bytes("data:text/plain;base64,aGk=").is_none());
        assert!(data_image_bytes("data:image/png,raw").is_none());
        assert!(data_image_bytes("https://x/y.png").is_none());
        let html = "<img src=\"https://x/y.png\">";
        let runs = html_runs_of(&[rep("text/html", html)], 3).expect("자리표시는 구조");
        assert_eq!(runs[0][0].text, "[image]");
        assert!(runs[0][0].image.is_none());
    }

    /// 심볼 치환·번호 목록·배율·PUA.
    #[test]
    fn symbols_ordered_lists_and_scale() {
        assert_eq!(sym_map(Sym::Wingdings, 'n'), '■');
        assert_eq!(sym_map(Sym::Wingdings, 'Ø'), '➢');
        assert_eq!(sym_map(Sym::Symbol, '·'), '•');
        assert_eq!(
            sym_map(Sym::None, '\u{F06E}'),
            '■',
            "PUA는 Wingdings로 본다"
        );
        assert_eq!(sym_map(Sym::None, 'n'), 'n');
        assert_eq!(sym_of("&quot;Wingdings&quot;, serif"), Sym::Wingdings);
        assert_eq!(sym_of("Wingdings 2"), Sym::Other);
        assert_eq!(scale_of("10pt"), Some(1.0));
        assert_eq!(scale_of("13.3333px"), Some(1.0));
        assert_eq!(scale_of("14pt"), Some(1.3), "상한");
        assert_eq!(scale_of("7pt"), Some(0.8), "하한");
        assert_eq!(len_em("2.5cm").map(|e| (e * 100.0).round()), Some(709.0));
        let html = "<ol><li style=\"color:#111\">a</li><li>b<ul><li>c</li></ul></li></ol>";
        let runs = html_runs_of(&[rep("text/html", html)], 9).expect("리치 런");
        let joined: Vec<String> = runs
            .iter()
            .map(|l| l.iter().map(|r| r.text.as_str()).collect())
            .collect();
        assert_eq!(joined, ["1. a", "2. b", "◦ c"], "{joined:?}");
        assert!(runs[2][0].indent > runs[1][0].indent);
        // 큰 제목은 배율이 실리고 본문은 1.0.
        let html2 = "<p style=\"font-size: 18pt\">T</p><p>body</p>";
        let runs2 = html_runs_of(&[rep("text/html", html2)], 9).expect("배율은 구조");
        assert!((runs2[0][0].scale - 1.3).abs() < f32::EPSILON);
        assert!((runs2[1][0].scale - 1.0).abs() < f32::EPSILON);
    }

    /// 개발용 — 실기 HTML 덤프를 런으로 풀어 찍는다(내용이 남의 데이터라 저장소에 넣지 않는다):
    /// `NCLIP_HTML_FIXTURE=<path> cargo test -p nclip-core dump_fixture -- --ignored --nocapture`.
    #[test]
    #[ignore = "NCLIP_HTML_FIXTURE 경로가 있을 때만"]
    fn dump_fixture() {
        let Ok(path) = std::env::var("NCLIP_HTML_FIXTURE") else {
            return;
        };
        let html = std::fs::read(path).expect("fixture 읽기");
        let reps = [crate::RawRep {
            format: "HTML Format".into(),
            data: html,
        }];
        let runs = html_runs_of(&reps, 200).expect("리치 런");
        for line in &runs {
            let text: String = line.iter().map(|r| r.text.as_str()).collect();
            let (ind, sc) = line.first().map_or((0.0, 1.0), |r| (r.indent, r.scale));
            println!("[{ind:>4.1}em ×{sc:.2}] {text}");
        }
    }

    /// style 본문은 글로 새지 않는다.
    #[test]
    fn style_block_skipped() {
        let html = "<style>.x{color:#fff}</style><span style=\"color:#111\">t</span>";
        let runs = html_runs_of(&[rep("CF_HTML", html)], 6).expect("리치 런");
        assert_eq!(runs[0].len(), 1);
        assert_eq!(runs[0][0].text, "t");
    }
}
