//! ★ 검색 색인(09-04 사용자 — "본문이 blob 파일에 흩어져 있으니 파일 I/O 병목 조치").
//!
//! 검색은 **RAM의 소문자 검색문**(라벨 + 본문 · 항목당 256KB 상한)만 훑는다 — 키 입력마다 파일을 열지 않는다.
//! - 캡처·편집·수신 때는 손에 있는 본문으로 바로 넣는다(비용 = 소문자 변환 1회).
//! - 기동 때 본문이 blob(16KB 이상)인 텍스트 항목은 **배경 스레드**가 `BlobReader`로 한 번 읽어 채운다
//!   (`ShellEvent::SearchText`) — 그동안은 라벨로만 걸러진다(점진 · UI 무정지 · DR-41).
//! - 삭제·일괄 삭제는 색인도 지운다. 규모: 291개 · 수 MB. 1만 건이라도 수십 MB 안(상한으로 묶인다).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 항목당 검색문 상한(바이트) — 그 뒤는 검색되지 않는다(문서 앞 256KB면 실용상 충분).
pub(crate) const TEXT_CAP: usize = 256 * 1024;

/// 메인·팝업·셸이 나눠 드는 색인.
pub(crate) type SearchIndex = Rc<RefCell<HashMap<u64, Rc<str>>>>;

pub(crate) fn new_index() -> SearchIndex {
    Rc::new(RefCell::new(HashMap::new()))
}
