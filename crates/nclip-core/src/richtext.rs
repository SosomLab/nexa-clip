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
//! - ★ 터미널 복사(09-04 사용자 — Windows Terminal "복사할 텍스트 형식 = HTML"): 블록 `white-space:pre` 존중 ·
//!   `font-family`가 고정폭이면 [`Run::mono`](Mono 슬롯) · `background-color` → [`Run::bg`] · 기울임.
//! - ★ ANSI SGR([`ansi_runs_of`]): 평문에 `ESC[…m`이 살아 있으면(원시 로그) 색·굵게·기울임을 런으로.
//! - ★ 표(10-05 · T-63): 행 = 줄 · 런마다 **표 번호·열 번호**([`Run::table`]·[`Run::cell`]) · 칸의 글자색·
//!   바탕색·오른쪽 맞춤. 열 폭 맞춤·칸 채움·테두리는 그리는 쪽이 이 번호로 한다. 병합(`colspan`/`rowspan`)은
//!   밖 — 병합 칸도 열 하나로 친다. 칸 안에서 줄이 바뀌면(여러 문단) 이어진 줄은 표 밖 줄(번호 0)로 나온다.

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
    /// ★ 고정폭(09-04 — 터미널·코드): 그리는 쪽이 Mono 슬롯 글꼴을 쓴다.
    pub mono: bool,
    /// ★ 런 배경색(09-04 — 터미널 검정 바탕 · 형광펜).
    pub bg: Option<[u8; 3]>,
    /// ★ 표의 열 번호(10-05 · T-63) — 0 = 표 밖 · n = 그 행의 n번째 칸(1부터). 병합 칸도 열 하나로 센다.
    pub cell: u16,
    /// ★ 표 번호 — 0 = 표 밖 · n = 이 문서의 n번째 표. 같은 번호의 줄들이 열 폭을 같이 쓴다
    /// (붙어 있는 두 표를 가르는 용도). `cell > 0`일 때만 뜻이 있다.
    pub table: u16,
    /// ★ 칸 안 오른쪽 맞춤(`align=right` · `text-align:right` — 스프레드시트의 숫자 칸). `cell > 0`일 때만.
    pub right: bool,
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
            mono: false,
            bg: None,
            cell: 0,
            table: 0,
            right: false,
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
    /// ★ 고정폭 글꼴명(터미널·코드) — Mono 슬롯.
    mono: bool,
    /// ★ 배경색.
    bg: Option<[u8; 3]>,
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
    /// ★ 블록 `white-space: pre*`(터미널 HTML은 DIV에 건다) — 자식 텍스트의 공백·개행 보존.
    pre: bool,
    /// ★ 블록 고정폭 글꼴 · 배경색(부모 상속).
    mono: bool,
    bg: Option<[u8; 3]>,
}

/// 블록 태그 `style`에서 읽은 값 묶음.
#[derive(Default)]
struct BlockAttrs {
    indent: Option<f32>,
    scale: Option<f32>,
    pre: Option<bool>,
    mono: Option<bool>,
    bg: Option<[u8; 3]>,
}

/// 표현 목록에서 HTML을 찾아 줄×런으로 푼다 — 없거나 못 풀면 `None`(평문 폴백).
///
/// `max_lines` 줄에서 끊는다(목록 행은 몇 줄만 필요 — 거대 문서 방어).
#[must_use]
pub fn html_runs_of(reps: &[crate::RawRep], max_lines: usize) -> Option<Vec<Vec<Run>>> {
    let Some(r) = reps
        .iter()
        // ★ 판별은 capture와 한 벌(09-04 mac 실기 — `public.html`이 빠져 PPT 서식이 평문으로).
        .find(|r| crate::capture::is_html_format(&r.format))
    else {
        // ★ HTML이 없으면 평문의 ANSI SGR(원시 로그 · 09-04) — ESC가 없으면 None.
        let best = reps
            .iter()
            .filter_map(|r| crate::capture::plain_rank(&r.format).map(|k| (k, r)))
            .min_by_key(|(k, _)| *k)?;
        let text = crate::capture::decode_plain(&best.1.format, &best.1.data)?;
        return ansi_runs_of(&text, max_lines);
    };
    let html = core::str::from_utf8(&r.data).ok()?;
    let (runs, structured) = parse(fragment_of(html), max_lines);
    // 색·서식이 하나도 없으면 평문과 같다 — 굳이 리치 경로를 태우지 않는다.
    // ★ 2단: 목록 불릿·들여쓰기·심볼 치환·이미지 자리표시는 평문이 잃은 구조라 리치로 친다.
    let styled = structured
        || runs.iter().flatten().any(|run| {
            run.color.is_some()
                || run.bold
                || run.italic
                || run.mono
                || run.bg.is_some()
                || (run.scale - 1.0).abs() > f32::EPSILON
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
    // ★ 표(T-63) — 지금 표 행에서 몇 번째 칸인가 · 표 번호(없으면 0) · 중첩 표 복원 스택.
    //   `row_line` = 이 행이 놓인 줄 — 칸 안에서 줄이 바뀌면 그 뒤 런은 표 밖(번호 0)으로 나간다.
    let mut cell = 0u32;
    let mut cur_table = 0u16;
    let mut next_table = 0u16;
    let mut table_stack: Vec<(u16, u32)> = Vec::new();
    let mut row_line = 0usize;
    let mut cell_right = false;
    // 칸 안 블록(`<td><p>…</p></td>` — Word) — 줄 바꿈을 **다음 글자가 올 때까지 미룬다**. 칸마다 문단이
    // 하나뿐인 흔한 표가 한 줄(한 행)로 남는다. 칸 밖에서는 종전대로 즉시 줄을 바꾼다.
    let mut in_cell = false;
    let mut cell_started = false;
    let mut pending_break = false;
    let mut text = String::new();
    let bytes = frag.as_bytes();
    let mut i = 0usize;
    // 직전이 공백이었나 — HTML 공백 붕괴(연속 공백·개행 = 한 칸). &nbsp;·탭은 보존.
    let mut last_ws = true;

    macro_rules! flush {
        () => {
            if !text.is_empty() {
                // (매크로가 펼쳐지는 자리에 따라 바로 뒤에서 다시 덮이는 대입이 있다 — 뜻은 맞다.)
                #[allow(unused_assignments)]
                {
                    if pending_break {
                        pending_break = false;
                        if !lines.last().is_none_or(Vec::is_empty) {
                            lines.push(Vec::new());
                        }
                    }
                    cell_started = in_cell;
                }
                let on_row = in_cell && lines.len() - 1 == row_line;
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
                let blk = blocks.last();
                line.push(Run {
                    text: core::mem::take(&mut text),
                    color: cur.color,
                    bold: cur.bold,
                    italic: cur.italic,
                    indent,
                    scale,
                    image: None,
                    mono: cur.mono || blk.is_some_and(|b| b.mono),
                    bg: cur.bg.or(blk.and_then(|b| b.bg)),
                    cell: if on_row {
                        u16::try_from(cell).unwrap_or(u16::MAX)
                    } else {
                        0
                    },
                    table: if on_row { cur_table } else { 0 },
                    right: on_row && cell_right,
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
                // ★ 표(T-63) — 표 번호를 새로 딴다(중첩이면 바깥 것을 쌓아 둔다). Excel 조각은 `<table>`이
                //   조각 밖이라 태그 없이 `<tr>`부터 온다 — 그때는 첫 칸에서 번호를 딴다.
                (false, "table") => {
                    flush!();
                    table_stack.push((cur_table, cell));
                    next_table = next_table.saturating_add(1);
                    cur_table = next_table;
                    cell = 0;
                    in_cell = false;
                    pending_break = false;
                }
                (true, "table") => {
                    flush!();
                    new_line!();
                    let (t, c) = table_stack.pop().unwrap_or((0, 0));
                    cur_table = t;
                    cell = c;
                    in_cell = false;
                    pending_break = false;
                    last_ws = true;
                }
                // 표 행 = 줄 바꿈.
                (_, "tr") => {
                    flush!();
                    new_line!();
                    last_ws = true;
                    cell = 0;
                    in_cell = false;
                    pending_break = false;
                }
                // ★ 표 칸(10-05 · T-63) — 런에 열 번호를 실어 그리는 쪽이 열을 맞추게 한다(칸 사이 탭은 없다).
                //   칸의 글자색·바탕색(`style`·`bgcolor`)·오른쪽 맞춤도 받는다. `colspan`은 보지 않는다(열 하나).
                (false, "td" | "th") => {
                    flush!();
                    if cur_table == 0 {
                        next_table = next_table.saturating_add(1);
                        cur_table = next_table;
                    }
                    if cell == 0 {
                        row_line = lines.len() - 1;
                    } else if lines.len() - 1 != row_line {
                        // 앞 칸에서 줄이 바뀌었다 — 이 칸은 행 줄에 못 놓는다(표 밖 줄). 종전처럼 탭으로 가른다.
                        text.push('\t');
                        flush!();
                    }
                    cell += 1;
                    marks += 1;
                    in_cell = true;
                    cell_started = false;
                    pending_break = false;
                    stack.push(cur);
                    apply_attrs(tag, &mut cur);
                    let low = tag.to_ascii_lowercase();
                    if let Some(c) =
                        attr_value(&low, tag, "bgcolor").and_then(|v| parse_color(v.trim()))
                    {
                        cur.bg = Some(c);
                    }
                    cell_right = cell_align_right(&low, tag);
                    if name == "th" {
                        cur.bold = true;
                    }
                    last_ws = true;
                }
                (true, "td" | "th") => {
                    flush!();
                    in_cell = false;
                    pending_break = false;
                    if let Some(prev) = stack.pop() {
                        cur = prev;
                    }
                }
                // ★ 2단: 블록 = 줄 바꿈 + 들여쓰기·배율 스택. 목록(ul/ol)은 줄을 바꾸지 않고 깊이만 더한다.
                (false, "div" | "p" | "li" | "h1" | "h2" | "h3" | "ul" | "ol") => {
                    flush!();
                    let is_list = matches!(name.as_str(), "ul" | "ol");
                    if !is_list {
                        if in_cell {
                            pending_break = cell_started;
                        } else {
                            new_line!();
                        }
                    }
                    let ba = block_attrs(tag);
                    let parent = blocks.last();
                    let (parent_scale, parent_pre, parent_mono, parent_bg) = parent
                        .map_or((1.0, false, false, None), |b| {
                            (b.scale, b.pre, b.mono, b.bg)
                        });
                    blocks.push(Block {
                        name: name.clone(),
                        // 목록 기본 들여쓰기 1.5em(브라우저 40px에 해당 — 행 폭이 좁아 조금 줄인다).
                        indent: ba.indent.unwrap_or(if is_list { 1.5 } else { 0.0 }),
                        scale: ba.scale.unwrap_or(parent_scale),
                        ordered: (name == "ol").then_some(0),
                        pre: ba.pre.unwrap_or(parent_pre),
                        mono: ba.mono.unwrap_or(parent_mono),
                        bg: ba.bg.or(parent_bg),
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
                        if in_cell {
                            pending_break = cell_started;
                        } else {
                            new_line!();
                        }
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
            // pre = 인라인(`<pre>`·span white-space) ∨ 블록(터미널 HTML의 DIV).
            let pre = cur.pre || blocks.last().is_some_and(|b| b.pre);
            for ch in decode_entities(&frag[i..next]).chars() {
                match ch {
                    '\r' => {}
                    // ★ pre 구간: 개행 = 줄 바꿈 · 공백 그대로(09-03 — Sublime계 HTML).
                    '\n' if pre => {
                        flush!();
                        lines.push(Vec::new());
                        last_ws = true;
                    }
                    ' ' if pre => {
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

/// ★ ONLYOFFICE 프레젠테이션 복사에 실린 **개체 자리**(10-04 사용자 실기 — "이미지로 복사하면 도형이 가로로 놓인다").
///
/// ONLYOFFICE는 개체를 `text/html`로 올리며 `<img>`(개체마다 그림 하나)에는 위치가 없다. 위치는
/// `class="pptData;<길이>;<base64>"`에 실린 자체 이진 안에만 있다. 그 이진에서 도형 변환(xfrm) 기록 —
/// 속성 묶음 `FA 00 <offX> 01 <offY> 02 <extX> 03 <extY>`(각 i32 LE · EMU) — 을 **순서대로** 찾아
/// `[offX, offY, extX, extY]`로 돌려준다. 전부 0인 기록(묶음 틀)과 크기가 없는 기록은 건너뛴다.
///
/// ⚠️ 문서화된 형식이 아니다 — 기록 수·크기가 그림과 맞는지는 쓰는 쪽이 확인하고, 안 맞으면
/// 위치를 쓰지 않는다(지어내지 않는다).
#[must_use]
pub fn onlyoffice_shape_rects(reps: &[crate::RawRep]) -> Vec<[i32; 4]> {
    const MAX: usize = 64;
    let Some(html) = reps
        .iter()
        .find(|r| crate::capture::is_html_format(&r.format))
        .and_then(|r| core::str::from_utf8(&r.data).ok())
    else {
        return Vec::new();
    };
    let Some(at) = html.find("pptData;") else {
        return Vec::new();
    };
    let rest = &html[at + "pptData;".len()..];
    let Some((_len, rest)) = rest.split_once(';') else {
        return Vec::new();
    };
    let payload = rest.split(['"', '\'', ' ', '>']).next().unwrap_or("");
    if payload.len() > 11 << 20 {
        return Vec::new();
    }
    let Some(bin) = base64_decode(payload) else {
        return Vec::new();
    };
    let int = |i: usize| i32::from_le_bytes([bin[i], bin[i + 1], bin[i + 2], bin[i + 3]]);
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 21 <= bin.len() && out.len() < MAX {
        if bin[i] == 0xFA
            && bin[i + 1] == 0
            && bin[i + 6] == 1
            && bin[i + 11] == 2
            && bin[i + 16] == 3
        {
            let r = [int(i + 2), int(i + 7), int(i + 12), int(i + 17)];
            if r[2] > 0 && r[3] > 0 {
                out.push(r);
            }
            i += 21;
        } else {
            i += 1;
        }
    }
    out
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

/// 블록 태그의 `style` — 들여쓰기(margin-left 단축형 포함 + padding-left + text-indent) · 배율 · pre · 고정폭 · 배경.
fn block_attrs(tag: &str) -> BlockAttrs {
    let low = tag.to_ascii_lowercase();
    let mut out = BlockAttrs::default();
    let Some(v) = attr_value(&low, tag, "style") else {
        return out;
    };
    let (mut indent, mut has) = (0.0f32, false);
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
            "font-size" => out.scale = scale_of(val),
            "white-space" => {
                let v = val.to_ascii_lowercase();
                out.pre = Some(v.starts_with("pre"));
            }
            "font-family" => out.mono = Some(is_mono_family(val)),
            "background-color" | "background" => {
                if let Some(c) = bg_color_of(val) {
                    out.bg = Some(c);
                }
            }
            _ => {}
        }
    }
    out.indent = has.then_some(indent.max(0.0));
    out
}

/// 고정폭 글꼴명인가 — 터미널·코드 편집기가 내보내는 이름들.
fn is_mono_family(v: &str) -> bool {
    let low = v.to_ascii_lowercase().replace("&quot;", "");
    const HINTS: [&str; 16] = [
        "mono",
        "consolas",
        "courier",
        "cascadia",
        "fira code",
        "source code",
        "menlo",
        "d2coding",
        "nerd font",
        "hack",
        "inconsolata",
        "iosevka",
        "nanum gothic coding",
        "nanumgothiccoding",
        "lucida console",
        "terminal",
    ];
    HINTS.iter().any(|h| low.contains(h))
}

/// `background`/`background-color` 값에서 색 토큰 하나(`#rgb` · `rgb()` · 단축형 첫 색). `transparent`·`none`은 None.
fn bg_color_of(v: &str) -> Option<[u8; 3]> {
    v.split_whitespace().find_map(parse_color)
}

/// ★ ANSI SGR(`ESC[…m`) → 줄×런(09-04 사용자 — 원시 로그). ESC가 없으면 `None`(평문 경로).
///
/// 지원: 0 리셋 · 1 굵게 · 3 기울임 · 22/23 해제 · 30~37/90~97 전경 · 40~47/100~107 배경 · 39/49 기본 ·
/// 38;5;n / 48;5;n(256색) · 38;2;r;g;b / 48;2;r;g;b(트루컬러). 그 밖의 CSI·OSC는 조용히 버린다.
/// 팔레트 = Windows Terminal Campbell(기본 색 16).
#[must_use]
pub fn ansi_runs_of(text: &str, max_lines: usize) -> Option<Vec<Vec<Run>>> {
    if !text.contains("\x1b[") {
        return None;
    }
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    let (mut fg, mut bg, mut bold, mut italic) = (None::<[u8; 3]>, None::<[u8; 3]>, false, false);
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                if let Some(line) = lines.last_mut() {
                    line.push(Run {
                        text: core::mem::take(&mut buf),
                        color: fg,
                        bold,
                        italic,
                        mono: true,
                        bg,
                        ..Run::default()
                    });
                }
            }
        };
    }
    while i < chars.len() && lines.len() <= max_lines {
        let c = chars[i];
        match c {
            '\x1b' => {
                match chars.get(i + 1) {
                    Some('[') => {
                        // CSI: 매개변수 … 종결 바이트(0x40~0x7E).
                        let start = i + 2;
                        let mut j = start;
                        while j < chars.len() && !('\u{40}'..='\u{7e}').contains(&chars[j]) {
                            j += 1;
                        }
                        if j < chars.len() && chars[j] == 'm' {
                            flush!();
                            let params: String = chars[start..j].iter().collect();
                            apply_sgr(&params, &mut fg, &mut bg, &mut bold, &mut italic);
                        }
                        i = j + 1;
                    }
                    Some(']') => {
                        // OSC: BEL 또는 ESC \ 까지.
                        let mut j = i + 2;
                        while j < chars.len()
                            && chars[j] != '\u{7}'
                            && !(chars[j] == '\x1b' && chars.get(j + 1) == Some(&'\\'))
                        {
                            j += 1;
                        }
                        i = if j < chars.len() && chars[j] == '\x1b' {
                            j + 2
                        } else {
                            j + 1
                        };
                    }
                    _ => i += 2,
                }
            }
            '\r' => i += 1,
            '\n' => {
                flush!();
                lines.push(Vec::new());
                i += 1;
            }
            c => {
                buf.push(c);
                i += 1;
            }
        }
    }
    flush!();
    while lines.last().is_some_and(Vec::is_empty) && lines.len() > 1 {
        lines.pop();
    }
    lines.truncate(max_lines);
    Some(lines)
}

fn apply_sgr(
    params: &str,
    fg: &mut Option<[u8; 3]>,
    bg: &mut Option<[u8; 3]>,
    bold: &mut bool,
    italic: &mut bool,
) {
    let ps: Vec<u16> = params
        .split([';', ':'])
        .map(|p| p.trim().parse::<u16>().unwrap_or(0))
        .collect();
    let ps = if ps.is_empty() { vec![0] } else { ps };
    let mut k = 0usize;
    while k < ps.len() {
        let p = ps[k];
        match p {
            0 => {
                *fg = None;
                *bg = None;
                *bold = false;
                *italic = false;
            }
            1 => *bold = true,
            3 => *italic = true,
            22 => *bold = false,
            23 => *italic = false,
            30..=37 => *fg = Some(ansi_palette(p - 30)),
            90..=97 => *fg = Some(ansi_palette(p - 90 + 8)),
            40..=47 => *bg = Some(ansi_palette(p - 40)),
            100..=107 => *bg = Some(ansi_palette(p - 100 + 8)),
            39 => *fg = None,
            49 => *bg = None,
            38 | 48 => {
                let target = if p == 38 { &mut *fg } else { &mut *bg };
                match ps.get(k + 1) {
                    Some(5) => {
                        if let Some(&n) = ps.get(k + 2) {
                            *target = Some(ansi_256(n));
                        }
                        k += 2;
                    }
                    Some(2) => {
                        if let (Some(&r), Some(&g), Some(&b)) =
                            (ps.get(k + 2), ps.get(k + 3), ps.get(k + 4))
                        {
                            *target = Some([r.min(255) as u8, g.min(255) as u8, b.min(255) as u8]);
                        }
                        k += 4;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        k += 1;
    }
}

/// 기본 16색 — Windows Terminal Campbell.
fn ansi_palette(i: u16) -> [u8; 3] {
    const P: [[u8; 3]; 16] = [
        [0x0C, 0x0C, 0x0C],
        [0xC5, 0x0F, 0x1F],
        [0x13, 0xA1, 0x0E],
        [0xC1, 0x9C, 0x00],
        [0x00, 0x37, 0xDA],
        [0x88, 0x17, 0x98],
        [0x3A, 0x96, 0xDD],
        [0xCC, 0xCC, 0xCC],
        [0x76, 0x76, 0x76],
        [0xE7, 0x48, 0x56],
        [0x16, 0xC6, 0x0C],
        [0xF9, 0xF1, 0xA5],
        [0x3B, 0x78, 0xFF],
        [0xB4, 0x00, 0x9E],
        [0x61, 0xD6, 0xD6],
        [0xF2, 0xF2, 0xF2],
    ];
    P[(i as usize).min(15)]
}

/// 256색: 0~15 기본 · 16~231 6×6×6 큐브 · 232~255 회색.
fn ansi_256(n: u16) -> [u8; 3] {
    match n {
        0..=15 => ansi_palette(n),
        16..=231 => {
            let v = n - 16;
            let step = |x: u16| -> u8 {
                if x == 0 {
                    0
                } else {
                    (55 + x * 40) as u8
                }
            };
            [step(v / 36), step((v / 6) % 6), step(v % 6)]
        }
        _ => {
            let g = (8 + (n.min(255) - 232) * 10) as u8;
            [g, g, g]
        }
    }
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
                // ★ 2단: 심볼 글꼴 · 인라인 글자 배율 · ★ 고정폭 · 배경(09-04 터미널).
                "font-family" => {
                    st.sym = sym_of(val);
                    st.mono = is_mono_family(val);
                }
                "background-color" | "background" => {
                    if let Some(c) = bg_color_of(val) {
                        st.bg = Some(c);
                    }
                }
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

/// 표 칸이 오른쪽 맞춤인가 — `align=right` 또는 `style`의 `text-align: right`.
fn cell_align_right(low: &str, tag: &str) -> bool {
    if attr_value(low, tag, "align").is_some_and(|v| v.trim().eq_ignore_ascii_case("right")) {
        return true;
    }
    attr_value(low, tag, "style").is_some_and(|v| {
        v.split(';').any(|d| {
            d.split_once(':').is_some_and(|(k, val)| {
                k.trim().eq_ignore_ascii_case("text-align")
                    && val.trim().eq_ignore_ascii_case("right")
            })
        })
    })
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
    // ★ 색 이름(09-04 mac PPT 실기 — 표준색은 `color:red`처럼 이름으로 온다) — CSS 기본 16 + 흔한 것.
    let named: Option<[u8; 3]> = match v.to_ascii_lowercase().as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([255, 0, 0]),
        "lime" => Some([0, 255, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 0, 255]),
        "navy" => Some([0, 0, 128]),
        "yellow" => Some([255, 255, 0]),
        "orange" => Some([255, 165, 0]),
        "purple" => Some([128, 0, 128]),
        "fuchsia" | "magenta" => Some([255, 0, 255]),
        "aqua" | "cyan" => Some([0, 255, 255]),
        "teal" => Some([0, 128, 128]),
        "olive" => Some([128, 128, 0]),
        "maroon" => Some([128, 0, 0]),
        "gray" | "grey" => Some([128, 128, 128]),
        "silver" => Some([192, 192, 192]),
        "darkgray" | "darkgrey" => Some([169, 169, 169]),
        "lightgray" | "lightgrey" => Some([211, 211, 211]),
        "brown" => Some([165, 42, 42]),
        "pink" => Some([255, 192, 203]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
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

    /// ★ 터미널 HTML(09-04 Windows Terminal 실기 축약본): DIV pre + 고정폭 글꼴 + 배경 · 공백 열 보존 · 기울임.
    #[test]
    fn terminal_html_keeps_columns_mono_and_bg() {
        let html = concat!(
            "<!--StartFragment --><DIV STYLE=\"display:inline-block;white-space:pre;background-color:#0C0C0C;",
            "font-family:'D2Coding, JetBrainsMono Nerd Font',monospace;font-size:12pt;padding:4px;\">",
            "<SPAN STYLE=\"color:#16C60C;background-color:#0C0C0C;\">Mode      Last</SPAN>",
            "<SPAN STYLE=\"color:#16C60C;background-color:#0C0C0C;font-style:italic;\">   Length</SPAN><BR>",
            "<SPAN STYLE=\"color:#CCCCCC;background-color:#0C0C0C;\">d----   2026</SPAN></DIV><!--EndFragment-->",
        );
        let runs = html_runs_of(&[rep("HTML Format", html)], 9).expect("리치 런");
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert_eq!(runs[0][0].text, "Mode      Last", "pre 블록 — 공백 열 보존");
        assert!(runs[0][0].mono, "고정폭");
        assert_eq!(runs[0][0].bg, Some([0x0C, 0x0C, 0x0C]));
        assert_eq!(runs[0][0].color, Some([0x16, 0xC6, 0x0C]));
        assert!(runs[0][1].italic);
        assert_eq!(runs[1][0].text, "d----   2026");
        assert!(is_mono_family("Consolas") && is_mono_family("&quot;Cascadia Mono&quot;"));
        assert!(!is_mono_family("맑은 고딕"));
    }

    /// ★ ANSI SGR — 16색·굵게·256·트루컬러·리셋·OSC 스킵 · ESC 없으면 None.
    #[test]
    fn ansi_sgr_to_runs() {
        assert!(ansi_runs_of("plain", 3).is_none());
        let t = "\x1b[1;32mOK\x1b[0m done\n\x1b[38;5;196mred\x1b[48;2;1;2;3m bg\x1b[0m\x1b]0;title\x07x";
        let runs = ansi_runs_of(t, 9).expect("ansi");
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert_eq!(runs[0][0].text, "OK");
        assert!(runs[0][0].bold && runs[0][0].mono);
        assert_eq!(runs[0][0].color, Some(ansi_palette(2)));
        assert_eq!(runs[0][1].text, " done");
        assert_eq!(runs[0][1].color, None);
        assert_eq!(runs[1][0].color, Some(ansi_256(196)));
        assert_eq!(runs[1][1].bg, Some([1, 2, 3]));
        assert_eq!(runs[1][2].text, "x", "OSC는 버린다");
        assert_eq!(ansi_256(231), [255, 255, 255]);
        assert_eq!(ansi_256(232), [8, 8, 8]);
        // reps 경로 — HTML 없고 평문에 ESC.
        let reps = [crate::RawRep {
            format: "text/plain".into(),
            data: b"\x1b[31mE\x1b[0m".to_vec(),
        }];
        assert_eq!(
            html_runs_of(&reps, 3).expect("ansi 폴백")[0][0].color,
            Some(ansi_palette(1))
        );
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

#[cfg(test)]
mod ppt_mac_tests {
    use super::*;

    /// ★ PPT(mac) 글상자 텍스트 — `public.html` · 작은따옴표 style · 태그 안 줄바꿈 ·
    /// 색 이름(`color:red`) · mso-* 잡음(09-04 실기 — 전부 평문으로 보이던 것).
    #[test]
    fn ppt_mac_public_html_keeps_colors() {
        let html = "<html><body><!--StartFragment-->\
<span style='font-size:44.0pt;font-family:\"맑은 고딕\";\nmso-ascii-font-family:\"Aptos Display\";color:black;mso-color-index:1'>가나다\n</span>\
<span style='font-size:44.0pt;\ncolor:red;mso-font-kerning:12.0pt'>123</span>\
<span\nstyle='font-size:44.0pt;color:#215F9A;mso-color-index:3'>ABC</span>\
<!--EndFragment--></body></html>";
        let reps = [crate::RawRep {
            format: "public.html".into(),
            data: html.as_bytes().to_vec(),
        }];
        let runs = html_runs_of(&reps, 5).expect("public.html은 리치로 잡혀야 한다");
        let flat: Vec<&Run> = runs.iter().flatten().collect();
        assert!(
            flat.iter()
                .any(|r| r.text.contains("123") && r.color == Some([255, 0, 0])),
            "색 이름 red가 살아야 한다: {flat:?}"
        );
        assert!(
            flat.iter()
                .any(|r| r.text.contains("ABC") && r.color == Some([0x21, 0x5F, 0x9A])),
            "hex 색이 살아야 한다: {flat:?}"
        );
    }
    /// ★ ONLYOFFICE `pptData` 이진에서 도형 자리(xfrm)를 순서대로 읽는다 — 0 기록·크기 없는 기록은 건너뛴다.
    #[test]
    fn onlyoffice_shape_rects_are_read_in_order() {
        let rec = |v: [i32; 4]| {
            let mut b = vec![0xFAu8];
            for (k, x) in v.iter().enumerate() {
                b.push(k as u8);
                b.extend_from_slice(&x.to_le_bytes());
            }
            b.push(0xFB);
            b
        };
        let mut bin = b"head".to_vec();
        bin.extend(rec([0, 0, 0, 0]));
        bin.extend(rec([1_469_385, 968_114, 2_389_057, 1_311_639]));
        bin.extend(b"\x01\x02junk");
        bin.extend(rec([2_929_191, 2_479_275, 2_857_500, 2_638_893]));
        // base64(표준) 인코드 — 테스트용 최소 구현.
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut b64 = String::new();
        for c in bin.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            b64.push(T[(n >> 18) as usize & 63] as char);
            b64.push(T[(n >> 12) as usize & 63] as char);
            b64.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            b64.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        let html = format!(
            "<img width=\"10\" height=\"10\" src=\"data:image/png;base64,AAAA\" class=\"pptData;{};{b64}\">",
            bin.len()
        );
        let reps = [crate::RawRep {
            format: "text/html".into(),
            data: html.into_bytes(),
        }];
        assert_eq!(
            onlyoffice_shape_rects(&reps),
            vec![
                [1_469_385, 968_114, 2_389_057, 1_311_639],
                [2_929_191, 2_479_275, 2_857_500, 2_638_893]
            ]
        );
        // pptData가 없으면 빈 목록.
        let plain = [crate::RawRep {
            format: "text/html".into(),
            data: b"<p>hello</p>".to_vec(),
        }];
        assert!(onlyoffice_shape_rects(&plain).is_empty());
    }
    fn html_rep(html: &str) -> [crate::RawRep; 1] {
        [crate::RawRep {
            format: "text/html".into(),
            data: html.as_bytes().to_vec(),
        }]
    }
    fn line_text(l: &[Run]) -> String {
        l.iter().map(|r| r.text.as_str()).collect()
    }

    /// ★ T-63 — 표 칸은 열 번호로 갈리고(탭 없음) 칸의 바탕색·글자색을 받는다.
    #[test]
    fn table_cells_carry_column_index_and_cell_colors() {
        let html = "<table><tr><th>1</th><th>2</th></tr>\
                    <tr><td>3</td><td bgcolor=\"#ff0000\" style=\"color:#ffffff\">4</td></tr></table>";
        let lines = html_runs_of(&html_rep(html), 10).expect("표는 서식 경로");
        assert_eq!(line_text(&lines[0]), "12");
        assert_eq!(line_text(&lines[1]), "34");
        let cells = |l: &[Run]| l.iter().map(|r| r.cell).collect::<Vec<_>>();
        assert_eq!(cells(&lines[0]), [1, 2]);
        assert_eq!(cells(&lines[1]), [1, 2]);
        assert!(lines[0].iter().all(|r| r.bold), "머리 칸은 굵게");
        let last = lines[1].last().expect("칸");
        assert_eq!(last.bg, Some([255, 0, 0]));
        assert_eq!(last.color, Some([255, 255, 255]));
        // 칸 바탕색은 옆 칸에 묻지 않는다.
        assert!(lines[1][0].bg.is_none());
    }

    /// ★ T-63 — 2행×3열 표의 열 번호 · 표 뒤 문단은 0 · 붙은 두 표는 번호가 다르다 · 오른쪽 맞춤.
    #[test]
    fn table_runs_have_column_and_table_ids() {
        let html = "<table><tr><td>a</td><td><b>b</b>x</td><td align=right>1</td></tr>\
                    <tr><td>d</td><td></td><td style=\"text-align: right\">22</td></tr></table>\
                    <p>after</p>\
                    <table><tr><td>p</td><td>q</td></tr></table>";
        let lines = html_runs_of(&html_rep(html), 10).expect("표");
        assert_eq!(lines.len(), 4);
        let cells = |l: &[Run]| l.iter().map(|r| r.cell).collect::<Vec<_>>();
        // 칸 안의 굵은 조각도 같은 열.
        assert_eq!(cells(&lines[0]), [1, 2, 2, 3]);
        // 빈 칸은 런이 없다 — 다음 칸의 번호는 건너뛴다.
        assert_eq!(cells(&lines[1]), [1, 3]);
        assert!(lines[0].iter().chain(&lines[1]).all(|r| r.table == 1));
        assert!(lines[0][3].right && lines[1][1].right && !lines[0][0].right);
        // 표 뒤 문단 = 표 밖.
        assert_eq!(line_text(&lines[2]), "after");
        assert!(lines[2]
            .iter()
            .all(|r| r.cell == 0 && r.table == 0 && !r.right));
        // 두 번째 표.
        assert_eq!(cells(&lines[3]), [1, 2]);
        assert!(lines[3].iter().all(|r| r.table == 2));
    }

    /// ★ T-63 — Excel 조각(`<table>` 없이 `<tr>`부터) · Word 칸(`<td><p>…</p></td>`)도 한 행 = 한 줄.
    #[test]
    fn table_without_table_tag_and_paragraph_cells() {
        let excel = "<tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr>";
        let lines = html_runs_of(&html_rep(excel), 10).expect("표");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().flatten().all(|r| r.table == 1 && r.cell > 0));

        let word = "<table><tr><td><p>a</p></td><td><p>b</p></td></tr>\
                    <tr><td><p>c</p><p>more</p></td><td><p>d</p></td></tr></table><p>tail</p>";
        let lines = html_runs_of(&html_rep(word), 10).expect("표");
        assert_eq!(line_text(&lines[0]), "ab");
        assert_eq!(lines[0].iter().map(|r| r.cell).collect::<Vec<_>>(), [1, 2]);
        // 칸 안에서 줄이 바뀌면 이어진 줄은 표 밖(번호 0 · 칸 사이는 탭) — 행은 첫 줄만.
        assert_eq!(line_text(&lines[1]), "c");
        assert_eq!(lines[1][0].cell, 1);
        assert_eq!(line_text(&lines[2]), "more\td");
        assert!(lines[2].iter().all(|r| r.cell == 0));
        assert_eq!(line_text(lines.last().expect("끝 줄")), "tail");
        assert_eq!(lines.last().expect("끝 줄")[0].cell, 0);
    }
}
