//! ★ 검색 방식 드롭다운(09-04 사용자 — "검색바 앞에 방식 3개를 고르는 정사각 아이콘 드롭다운").
//!
//! beep의 [`IconDropdown`](nclip_ctl::controls::IconDropdown)(19번째 컨트롤 · 알파 마스크 아이콘 + 라벨 목록)을 그대로 쓴다.
//! 아이콘은 자산 파일 대신 **UI 글꼴로 글리프를 구워** 만든다(96² 알파 · 한 번) — 정확히 `Aa` · 유사 `≈` · 정규식 `.*`.
//! 값은 설정 `find.mode`와 1:1(`exact` · `fuzzy` · `regex`) — 고르면 셸이 설정에 쓰고, 박동 동기가 두 창을 다시 거른다.

use nclip_core::{current_lang, tr, Msg};
use nclip_ctl::controls::{IconDropItem, IconDropdown};
use nclip_gfx::{Color, Font, Surface, TextStyle};

/// 마스크 변(px) — 툴바 알파 아이콘과 같은 규약.
pub(crate) const SIDE: u32 = 96;

/// 글리프 → 96² 알파(가운데 정렬 · 굵게). 한 번 굽고 누수시켜 `'static`으로(항목 3 × 창 2 · 9KB씩).
fn glyph_alpha(font: &Font, text: &str, size: f32) -> &'static [u8] {
    let side = SIDE as usize;
    let mut buf = vec![0u32; side * side];
    let mut surf = Surface::new(&mut buf, side, side);
    let w = font.measure(text, size);
    #[allow(clippy::cast_precision_loss)]
    let x = ((SIDE as f32 - w) / 2.0).max(0.0);
    #[allow(clippy::cast_precision_loss)]
    let y = (SIDE as f32 - font.line_height(size)) / 2.0 + font.ascent(size);
    #[allow(clippy::cast_possible_wrap)]
    let clip = (0, 0, SIDE as i32, SIDE as i32);
    font.draw_styled(
        &mut surf,
        x,
        y,
        size,
        Color::from_rgb(255, 255, 255),
        text,
        clip,
        TextStyle {
            bold: true,
            italic: false,
        },
    );
    // 검정 바탕에 흰 글자 → R 채널이 곧 덮임(알파).
    let alpha: Vec<u8> = buf.iter().map(|p| (p >> 16) as u8).collect();
    Box::leak(alpha.into_boxed_slice())
}

/// 드롭다운 생성 — `value` = 현재 `find.mode`.
pub(crate) fn build(font: &Font, value: &str) -> IconDropdown {
    let lang = current_lang();
    let items = vec![
        IconDropItem {
            value: "exact",
            label: tr(lang, Msg::ValExact).to_string(),
            alpha: glyph_alpha(font, "Aa", 56.0),
            size: SIDE,
        },
        IconDropItem {
            value: "fuzzy",
            label: tr(lang, Msg::ValFuzzy).to_string(),
            alpha: glyph_alpha(font, "≈", 84.0),
            size: SIDE,
        },
        IconDropItem {
            value: "regex",
            label: tr(lang, Msg::ValRegex).to_string(),
            alpha: glyph_alpha(font, ".*", 66.0),
            size: SIDE,
        },
    ];
    IconDropdown::new(items, value)
}
