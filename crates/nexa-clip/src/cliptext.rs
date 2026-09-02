//! 검색창 클립보드 연동(09-01 — 캐럿·선택 편집 요청) — 텍스트 한 줄 넣고/빼기.
//!
//! 재적재(`set_reps`)와 같은 경로라 감시가 에코를 볼 수 있다 — 검색어 복사는 사용자
//! 명시 행위이므로 이력에 잡혀도 정직하다(숨기지 않는다).

/// 선택 텍스트를 클립보드로.
pub(crate) fn set_text(text: &str) {
    let _ = nclip_plat::clipboard::set_reps(&nclip_plat::clipboard::plain_text_reps(text));
}

/// 지금 클립보드의 평문(없으면 None).
pub(crate) fn get_text() -> Option<String> {
    nclip_plat::watch::PlatformWatch::new()
        .read_now()
        .and_then(|s| s.plain_text())
}

/// 클립보드에 평문이 있는가 — 우클릭 편집 메뉴의 "붙여넣기" 활성 근거.
pub(crate) fn has_text() -> bool {
    get_text().is_some()
}
