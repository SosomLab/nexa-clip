//! ★ 서식 보기의 **표 배치**(10-05 · T-63) — 열 폭 맞춤 · 칸 채움 · 테두리.
//!
//! 파서([`nclip_core::richtext`])는 런마다 표 번호·열 번호([`Run::table`]·[`Run::cell`])만 실어 준다.
//! 여기서는 **주어진 줄들만 보고**(목록 행은 앞 몇 줄 · 미리보기는 전부) 표마다 열 폭 = 그 열의 가장 넓은
//! 내용 + 좌우 여백을 구해, 줄마다 칸 사각형과 런의 x 자리를 돌려준다. 폭 재기는 호출부가 클로저로 넘긴다 —
//! 창 없이 시험할 수 있고, 메인 목록 · 미리보기 · 팝업 · 이미지 내보내기가 **한 계산**을 쓴다.
//!
//! ## 범위 밖
//! - 병합(`colspan`/`rowspan`) — 병합 칸도 열 하나로 친다(그 열이 넓어질 뿐 어긋나지는 않는다).
//! - 칸 안 줄바꿈 — 파서가 이어진 줄을 표 밖 줄로 내보낸다(행은 첫 줄만 표로 그린다).
//! - 열이 하나뿐인 표(메일의 배치용 표가 대부분) — 표로 그리지 않는다(문단마다 상자가 쳐지는 것을 막는다).
//! - 런이 없는 빈 칸의 바탕색 — 칸 테두리는 그리지만 채움은 없다.

use nclip_core::richtext::Run;
use nclip_ctl::draw::DrawCtx;
use nclip_ctl::geom::Rect;
use nclip_ctl::theme::Color;

/// 표 한 칸 — x·폭은 **내용 원점 기준**(px). 높이는 줄 높이(그리는 쪽이 안다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    /// 칸 왼쪽 끝(왼쪽 테두리 자리).
    pub x: i32,
    /// 칸 폭(= 열 폭).
    pub w: i32,
    /// 칸 채움색 — 칸의 첫 런 바탕색.
    pub bg: Option<[u8; 3]>,
}

/// 표 한 행(= 한 줄)의 배치.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    /// 표 왼쪽 끝(내용 원점 기준).
    pub x: i32,
    /// 표 전체 폭 — 오른쪽 테두리는 `x + w`에 선다.
    pub w: i32,
    /// 열 순서대로의 칸(이 행에 런이 없는 열도 포함 — 격자가 끊기지 않게).
    pub cells: Vec<Cell>,
    /// 줄의 런마다 글자를 시작할 x(내용 원점 기준 · 여백·오른쪽 맞춤 반영).
    pub run_x: Vec<i32>,
}

/// 줄이 표 행인가 — 런 하나라도 열 번호가 있으면 행이다.
fn table_of(line: &[Run]) -> Option<u16> {
    line.iter().find(|r| r.cell > 0).map(|r| r.table)
}

/// 표 배치를 구한다 — 반환은 줄마다 `Some(행 배치)`(표 행) / `None`(보통 줄). 표가 없으면 **빈 Vec**.
///
/// - `em` = 본문 전각 폭(표의 들여쓰기 — 표 첫 행 첫 런의 [`Run::indent`]).
/// - `pad` = 칸 좌우 여백(px).
/// - `measure(줄, 런, 런)` = 그 런의 그려질 폭(px) — 글꼴 선택은 호출부 몫.
pub(crate) fn layout(
    lines: &[Vec<Run>],
    em: i32,
    pad: i32,
    mut measure: impl FnMut(usize, usize, &Run) -> i32,
) -> Vec<Option<Row>> {
    if !lines.iter().any(|l| table_of(l).is_some()) {
        return Vec::new();
    }
    /// 표 하나의 집계 — 번호 · 열별 최대 내용 폭 · 들여쓰기(px).
    struct Table {
        id: u16,
        widths: Vec<i32>,
        x: i32,
    }
    /// 행 하나의 실측 — 표 색인 · 런별 (열(0부터), 폭).
    struct Measured {
        table: usize,
        runs: Vec<(usize, i32)>,
    }
    let mut tables: Vec<Table> = Vec::new();
    let mut rows: Vec<Option<Measured>> = Vec::with_capacity(lines.len());
    for (li, line) in lines.iter().enumerate() {
        let Some(id) = table_of(line) else {
            rows.push(None);
            continue;
        };
        let ti = tables.iter().position(|t| t.id == id).unwrap_or_else(|| {
            tables.push(Table {
                id,
                widths: Vec::new(),
                x: nclip_core::richtext::em_px(em, line.first().map_or(0.0, |r| r.indent)),
            });
            tables.len() - 1
        });
        // 열은 왼쪽에서 오른쪽으로만 간다 — 번호 없는 런(0)·거꾸로 가는 번호는 직전 열에 붙인다.
        let mut col = 0usize;
        let mut runs = Vec::with_capacity(line.len());
        let mut sums: Vec<i32> = Vec::new();
        for (ri, run) in line.iter().enumerate() {
            col = col.max(usize::from(run.cell).saturating_sub(1));
            let w = measure(li, ri, run).max(0);
            if sums.len() <= col {
                sums.resize(col + 1, 0);
            }
            sums[col] += w;
            runs.push((col, w));
        }
        let t = &mut tables[ti];
        if t.widths.len() < sums.len() {
            t.widths.resize(sums.len(), 0);
        }
        for (w, s) in t.widths.iter_mut().zip(&sums) {
            *w = (*w).max(*s);
        }
        rows.push(Some(Measured { table: ti, runs }));
    }
    lines
        .iter()
        .zip(rows)
        .map(|(line, m)| {
            let m = m?;
            let t = &tables[m.table];
            // 열 하나짜리는 표로 그리지 않는다(배치용 표).
            if t.widths.len() < 2 {
                return None;
            }
            let mut x = t.x;
            let mut cells: Vec<Cell> = t
                .widths
                .iter()
                .map(|cw| {
                    let c = Cell {
                        x,
                        w: cw + pad * 2,
                        bg: None,
                    };
                    x += c.w;
                    c
                })
                .collect();
            // 칸마다 내용 폭(오른쪽 맞춤용) · 채움색(첫 런).
            let mut content = vec![0i32; cells.len()];
            let mut seen = vec![false; cells.len()];
            for ((col, w), run) in m.runs.iter().zip(line) {
                content[*col] += w;
                if !seen[*col] {
                    seen[*col] = true;
                    cells[*col].bg = run.bg;
                }
            }
            let mut cursor: Vec<Option<i32>> = vec![None; cells.len()];
            let run_x = m
                .runs
                .iter()
                .zip(line)
                .map(|((col, w), run)| {
                    let c = &cells[*col];
                    let at = cursor[*col].get_or_insert(if run.right {
                        c.x + c.w - pad - content[*col]
                    } else {
                        c.x + pad
                    });
                    let here = *at;
                    *at += w;
                    here
                })
                .collect();
            Some(Row {
                x: t.x,
                w: x - t.x,
                cells,
                run_x,
            })
        })
        .collect()
}

/// 행의 **테두리 선**(왼쪽 위 x·y·폭·높이) — 위·아래 가로선 + 칸마다 왼쪽 세로선 + 표 오른쪽 끝 세로선.
/// 아래 가로선은 다음 행의 위 선과 같은 자리라 겹쳐 그려도 한 줄이다.
pub(crate) fn border_rects(
    row: &Row,
    h: i32,
    bw: i32,
) -> impl Iterator<Item = (i32, i32, i32, i32)> + '_ {
    let horiz = [(row.x, 0, row.w + bw, bw), (row.x, h, row.w + bw, bw)];
    let vert = row
        .cells
        .iter()
        .map(|c| c.x)
        .chain(core::iter::once(row.x + row.w))
        .map(move |x| (x, 0, bw, h + bw));
    horiz.into_iter().chain(vert)
}

/// 화면(창) 쪽 공용 — 행의 칸 채움과 테두리를 그린다. `(ox, y)` = 내용 원점 · 줄 위, `h` = 줄 높이,
/// `bw` = 선 굵기(px). 전부 `clip` 안으로 자른다. 글자는 호출부가 [`Row::run_x`] 자리에 그린다.
pub(crate) fn paint_frame<D: DrawCtx + ?Sized>(
    dc: &mut D,
    row: &Row,
    (ox, y): (i32, i32),
    h: i32,
    clip: Rect,
    border: Color,
    bw: i32,
) {
    let cut = crate::main_win::clip_to;
    for c in &row.cells {
        if let Some(b) = c.bg {
            dc.fill_rect(
                cut(Rect::new(ox + c.x, y, c.w, h), clip),
                Color::from_rgb(b[0], b[1], b[2]),
            );
        }
    }
    for (bx, by, w, bh) in border_rects(row, h, bw) {
        let r = cut(Rect::new(ox + bx, y + by, w, bh), clip);
        if r.w > 0 && r.h > 0 {
            dc.fill_rect(r, border);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_run(text: &str, cell: u16, table: u16) -> Run {
        Run {
            text: text.into(),
            cell,
            table,
            ..Run::default()
        }
    }
    /// 가짜 자 — 글자 수 × 5px.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn fake(_: usize, _: usize, r: &Run) -> i32 {
        r.text.chars().count() as i32 * 5
    }

    /// ★ 두 행의 칸 폭이 [10, 30] · [25, 5] → 열 시작·폭이 두 행에서 같고 = 최대 + 여백×2.
    #[test]
    fn columns_align_across_rows() {
        let lines = vec![
            vec![cell_run("ab", 1, 1), cell_run("abcdef", 2, 1)],
            vec![cell_run("abcde", 1, 1), cell_run("a", 2, 1)],
        ];
        let out = layout(&lines, 10, 4, fake);
        let (a, b) = (
            out[0].as_ref().expect("표 행"),
            out[1].as_ref().expect("표 행"),
        );
        assert_eq!(a.cells, b.cells, "열 자리·폭은 행마다 같다");
        assert_eq!(
            a.cells.iter().map(|c| (c.x, c.w)).collect::<Vec<_>>(),
            [(0, 25 + 8), (33, 30 + 8)]
        );
        assert_eq!((a.x, a.w), (0, 33 + 38));
        // 글자는 칸 왼쪽 + 여백에서.
        assert_eq!(a.run_x, [4, 37]);
        assert_eq!(b.run_x, [4, 37]);
    }

    /// 표 밖 줄은 None · 표가 없으면 빈 Vec · 열 하나짜리는 표가 아니다 · 붙은 두 표는 폭을 따로 쓴다.
    #[test]
    fn non_table_lines_and_separate_tables() {
        let plain = vec![vec![cell_run("x", 0, 0)]];
        assert!(layout(&plain, 10, 4, fake).is_empty());

        let one_col = vec![vec![cell_run("x", 1, 1)], vec![cell_run("y", 1, 1)]];
        assert!(layout(&one_col, 10, 4, fake).iter().all(Option::is_none));

        let lines = vec![
            vec![cell_run("aaaa", 1, 1), cell_run("b", 2, 1)],
            vec![cell_run("para", 0, 0)],
            vec![cell_run("c", 1, 2), cell_run("d", 2, 2)],
        ];
        let out = layout(&lines, 10, 4, fake);
        assert!(out[1].is_none());
        assert_eq!(out[0].as_ref().expect("표1").cells[0].w, 20 + 8);
        assert_eq!(out[2].as_ref().expect("표2").cells[0].w, 5 + 8);
    }

    /// 빈 칸(런 없는 열)도 칸이 생긴다 · 한 칸의 여러 런은 이어 놓인다 · 오른쪽 맞춤 · 채움색 · 들여쓰기.
    #[test]
    fn gaps_multi_runs_right_align_and_fill() {
        let mut right = cell_run("1", 3, 1);
        right.right = true;
        let mut filled = cell_run("ab", 1, 1);
        filled.bg = Some([1, 2, 3]);
        filled.indent = 2.0;
        let lines = vec![
            vec![filled, cell_run("cd", 1, 1), right],
            vec![
                cell_run("x", 1, 1),
                cell_run("yy", 2, 1),
                cell_run("zzzz", 3, 1),
            ],
        ];
        let out = layout(&lines, 10, 4, fake);
        let a = out[0].as_ref().expect("표 행");
        // 들여쓰기 2em = 20px. 열 폭 = [20, 10, 20] + 8.
        assert_eq!(
            a.cells.iter().map(|c| (c.x, c.w)).collect::<Vec<_>>(),
            [(20, 28), (48, 18), (66, 28)]
        );
        assert_eq!(a.cells[0].bg, Some([1, 2, 3]));
        assert_eq!(a.cells[1].bg, None);
        // 첫 칸의 두 런은 이어서 · 셋째 칸은 오른쪽 끝 − 여백 − 폭(5).
        assert_eq!(a.run_x, [24, 34, 66 + 28 - 4 - 5]);
        // 테두리 — 세로선은 칸 왼쪽마다 + 표 오른쪽 끝.
        let xs: Vec<i32> = border_rects(a, 22, 1)
            .filter(|r| r.2 == 1)
            .map(|r| r.0)
            .collect();
        assert_eq!(xs, [20, 48, 66, 94]);
    }
}
