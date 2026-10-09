//! ★ 항목 종류 아이콘(10-04 사용자 요청) — 목록에 저장된 것이 **파일인지 · 이미지인지 ·
//! 서식 글인지 · 일반 글인지** 한눈에 갈리게 그림으로 표시한다.
//!
//! 종전에는 글리프 한 글자(`▤ ▧ ▣ ▦ ◆ ◇`)였는데 서로 닮아 구분이 안 됐다.
//! 글꼴에 기대지 않고 **도형으로 직접 그린다**(DR-1 — 3-OS 동일 화면 · 두부 없음).
//! 메인창과 팝업이 같은 함수를 쓴다.
//!
//! | 종류 | 그림 |
//! |---|---|
//! | 일반 글 | 같은 굵기의 줄 셋 |
//! | 서식 글 | 굵은 제목 줄 + 가는 줄 둘 |
//! | 이미지 | 액자 안의 산과 해 |
//! | 파일 | 귀 접힌 종이 |
//! | 색 | 둥근 색 조각 |
//! | 앱 개체 | 네모 테두리 + 겹친 원(도형) |

use nclip_core::ClipKind;
use nexa_ctl::draw::DrawCtx;
use nexa_ctl::geom::Rect;
use nexa_ctl::theme::Color;

/// `(x, y)`에서 한 변 `side`인 정사각 상자 안에 종류 아이콘을 그린다.
///
/// 좌표는 16칸 격자로 잡고 `side`에 맞춰 늘린다 — 배율(DPI)이 달라도 비례가 같다.
pub(crate) fn draw<D: DrawCtx + ?Sized>(
    dc: &mut D,
    x: i32,
    y: i32,
    side: i32,
    kind: ClipKind,
    ink: Color,
) {
    // 격자 칸 → 화소. 선 굵기는 1화소 밑으로 내려가지 않게.
    let u = |v: i32| side * v / 16;
    let t = (side / 10).max(1);
    let bar = |dc: &mut D, gx: i32, gy: i32, gw: i32, th: i32| {
        dc.fill_rect(
            Rect::new(x + u(gx), y + u(gy), u(gw).max(1), th.max(1)),
            ink,
        );
    };
    let frame = |dc: &mut D, gx: i32, gy: i32, gw: i32, gh: i32| {
        let (fx, fy, fw, fh) = (x + u(gx), y + u(gy), u(gw), u(gh));
        dc.fill_rect(Rect::new(fx, fy, fw, t), ink);
        dc.fill_rect(Rect::new(fx, fy + fh - t, fw, t), ink);
        dc.fill_rect(Rect::new(fx, fy, t, fh), ink);
        dc.fill_rect(Rect::new(fx + fw - t, fy, t, fh), ink);
    };
    match kind {
        ClipKind::Text => {
            let th = u(2);
            bar(dc, 2, 3, 12, th);
            bar(dc, 2, 7, 12, th);
            bar(dc, 2, 11, 8, th);
        }
        ClipKind::RichText => {
            bar(dc, 2, 2, 12, u(4));
            bar(dc, 2, 9, 12, t);
            bar(dc, 2, 12, 8, t);
        }
        ClipKind::Image => {
            frame(dc, 1, 2, 14, 12);
            dc.fill_triangle(
                (x + u(3), y + u(12)),
                (x + u(7), y + u(6)),
                (x + u(11), y + u(12)),
                ink,
            );
            dc.fill_rect(
                Rect::new(x + u(10), y + u(4), u(2).max(1), u(2).max(1)),
                ink,
            );
        }
        ClipKind::Files => {
            // 종이 — 왼쪽·아래·오른쪽(접힌 귀 아래부터)·위(접힌 귀 앞까지) + 접힌 귀.
            let (l, r, top, bot, ear) = (x + u(3), x + u(13), y + u(1), y + u(15), u(4));
            dc.fill_rect(Rect::new(l, top, t, bot - top), ink);
            dc.fill_rect(Rect::new(l, bot - t, r - l, t), ink);
            dc.fill_rect(Rect::new(r - t, top + ear, t, bot - top - ear), ink);
            dc.fill_rect(Rect::new(l, top, r - l - ear, t), ink);
            dc.fill_triangle((r - ear, top), (r - ear, top + ear), (r, top + ear), ink);
        }
        ClipKind::Color => {
            dc.fill_round_rect(Rect::new(x + u(2), y + u(2), u(12), u(12)), u(6), ink);
        }
        ClipKind::Object => {
            frame(dc, 1, 1, 10, 10);
            dc.fill_round_rect(Rect::new(x + u(7), y + u(7), u(8), u(8)), u(4), ink);
        }
    }
}
