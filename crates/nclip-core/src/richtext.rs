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
//! - 표·목록 구조·배경색·글꼴 크기는 본편(T-18d)

/// 스타일 런 — 같은 스타일이 이어지는 텍스트 조각.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Run {
    /// 조각 텍스트(엔티티 해제 후).
    pub text: String,
    /// 글자색(RGB) — 없으면 테마 본문색.
    pub color: Option<[u8; 3]>,
    /// 굵게.
    pub bold: bool,
    /// 기울임.
    pub italic: bool,
}

#[derive(Clone, Copy, Default)]
struct Style {
    color: Option<[u8; 3]>,
    bold: bool,
    italic: bool,
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
    let runs = parse(fragment_of(html), max_lines);
    // 색·서식이 하나도 없으면 평문과 같다 — 굳이 리치 경로를 태우지 않는다.
    let styled = runs
        .iter()
        .flatten()
        .any(|run| run.color.is_some() || run.bold || run.italic);
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
fn parse(frag: &str, max_lines: usize) -> Vec<Vec<Run>> {
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    let mut stack: Vec<Style> = Vec::new();
    let mut cur = Style::default();
    let mut text = String::new();
    let bytes = frag.as_bytes();
    let mut i = 0usize;
    // 직전이 공백이었나 — HTML 공백 붕괴(연속 공백·개행 = 한 칸). &nbsp;·탭은 보존.
    let mut last_ws = true;

    macro_rules! flush {
        () => {
            if !text.is_empty() {
                let line = lines.last_mut().unwrap_or_else(|| unreachable!());
                line.push(Run {
                    text: core::mem::take(&mut text),
                    color: cur.color,
                    bold: cur.bold,
                    italic: cur.italic,
                });
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
                // 블록 시작·끝 = 줄 바꿈(빈 줄 중복은 만들지 않는다).
                (_, "div" | "p" | "li" | "tr" | "h1" | "h2" | "h3" | "pre") => {
                    flush!();
                    if !lines.last().is_none_or(Vec::is_empty) {
                        lines.push(Vec::new());
                    }
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
                        text.push(c);
                        last_ws = c == '\t';
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
    lines
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
            let (k, val) = (k.trim(), val.trim());
            match k {
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
                "font-style" => {
                    if val.eq_ignore_ascii_case("italic") {
                        st.italic = true;
                    } else if val.eq_ignore_ascii_case("normal") {
                        st.italic = false;
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
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
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

    /// style 본문은 글로 새지 않는다.
    #[test]
    fn style_block_skipped() {
        let html = "<style>.x{color:#fff}</style><span style=\"color:#111\">t</span>";
        let runs = html_runs_of(&[rep("CF_HTML", html)], 6).expect("리치 런");
        assert_eq!(runs[0].len(), 1);
        assert_eq!(runs[0][0].text, "t");
    }
}
