//! 메모리 이력 — ★ **잡은 것을 들고 있다가 승격·상한을 적용한다**(T-13).
//!
//! 시작 때 저장소(T-16 · [`from_items`](History::from_items))에서 복원되고, 변화는 셸이
//! id로 짝을 맞춰 이벤트로 흘려보낸다(이력 자체는 여전히 순수 메모리 — 디스크를 모른다):
//!
//! | 규칙 | 왜 |
//! |---|---|
//! | ★ **같은 내용 재복사 = 새 항목이 아니라 승격** | 목록이 같은 것으로 도배되면 이력이 아니다([docs/20 §3-2]) |
//! | 상한 초과 = 오래된 것부터 제거 | `store.max_items` — 무한히 크는 목록은 상주 예산의 적(DR-9) |
//! | ★ 연속 장면([`coalesces`])은 **교체** | 탐색기 재게시·플러시 중간이 항목을 불리면 안 된다(D-80) |
//!
//! 내용 동일성은 **정렬된 (표현 이름, 바이트) 전체**다 — 미리보기 문자열 비교는
//! 다른 내용을 같다고 오판한다(같은 첫 줄, 다른 본문). 매번 전량 비교하면 이미지에서
//! 비싸므로 **FNV-1a 지문으로 거르고 바이트로 확정**한다.

use crate::capture::coalesces_parts;
use crate::{ClipKind, ClipSnapshot, RawRep};
use std::collections::VecDeque;

/// 이력 항목 — 표현 원본을 통째로 든다(재적재가 존재 이유다).
#[derive(Clone, Debug)]
pub struct HistoryItem {
    /// ★ 항목 id — 셸이 영속(T-16)과 짝을 맞추는 열쇠. `push`가 단조 증가로 부여하고,
    ///   복원([`History::from_items`])된 항목은 저장돼 있던 값을 그대로 쓴다.
    pub id: u64,
    /// ★ 핀(T-18b0 기초) — 상한 축출에서 지켜진다. UI는 후속.
    pub pinned: bool,
    /// 종류.
    pub kind: ClipKind,
    /// 목록·메뉴에 보일 한 줄(호출자가 만든다 — 이력은 미리보기 정책을 모른다).
    pub label: String,
    /// 표현 원본 — ★ **재적재 때 전부 되돌린다**(P-2 · 원본 붙여넣기).
    pub reps: Vec<RawRep>,
    /// 출처 앱.
    pub source_app: Option<String>,
    /// 복사된 횟수(승격마다 +1).
    pub copies: u32,
    /// ★ 목록용 썸네일(w, h, RGBA) — 호출자가 만든다(이력은 디코더를 모른다).
    ///   `None` = 이미지 아님·미리보기 꺼짐·디코드 실패(목록은 글리프 폴백).
    pub thumb: Option<(u32, u32, Vec<u8>)>,
    /// ★ 생성 시각(epoch ms) — 보관 기간 정책(T-13)의 근거. 구본 복원은 0(= 기간 면제 —
    ///   모르는 나이로 지우지 않는다 · fail-soft).
    pub created_ms: u64,
    /// 표현 바이트 합계 캠시 — 총용량 예산(500MB 기본) 판정이 매번 재지 않게.
    bytes: u64,
    /// 내용 지문(빠른 동일성 후보 판정).
    fingerprint: u64,
}

/// 벽시계 epoch ms — 보관 기간 판정은 달력 시간이 맞다(단조 시계 아님).
fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn reps_bytes(reps: &[RawRep]) -> u64 {
    reps.iter().map(|r| r.data.len() as u64).sum()
}

/// ★ 휘발 벤더 토큰(08-29 mac 실기 — `ole.source.0x…`가 복사마다 다르다) —
/// 동일성 비교에서 제외한다. 안 빼면 같은 복사가 매번 새 항목이 돼 승격이 죽는다.
fn is_volatile_format(fmt: &str) -> bool {
    fmt.starts_with("ole.source.")
}

impl HistoryItem {
    /// 저장소에서 복원할 때의 생성자 — 지문은 여기서 재계산한다(디스크의 지문을 믿지 않는다).
    #[must_use]
    #[allow(clippy::too_many_arguments)] // 영속 형상 1:1 — 묶으면 오히려 한 겹 더 생긴다
    #[allow(clippy::too_many_arguments)]
    pub fn restored(
        id: u64,
        kind: ClipKind,
        label: String,
        reps: Vec<RawRep>,
        source_app: Option<String>,
        copies: u32,
        pinned: bool,
        thumb: Option<(u32, u32, Vec<u8>)>,
        created_ms: u64,
    ) -> Self {
        let fingerprint = fingerprint(&reps);
        let bytes = reps_bytes(&reps);
        Self {
            id,
            pinned,
            kind,
            label,
            reps,
            source_app,
            copies,
            thumb,
            created_ms,
            bytes,
            fingerprint,
        }
    }
}

/// [`History::push`]의 결과 — 호출자가 화면 갱신 여부를 판단한다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pushed {
    /// 새 항목이 맨 위에 들어갔다.
    New,
    /// ★ 같은 내용이 이미 있어 **맨 위로 승격**됐다(`copies` +1).
    Promoted,
    /// ★ 맨 위 항목의 **다음 장면**이라 교체됐다(재게시·플러시 완본 — D-80).
    Replaced,
}

/// 세션 이력 — 최신이 앞(인덱스 0).
#[derive(Debug)]
pub struct History {
    items: VecDeque<HistoryItem>,
    cap: usize,
    /// ★ 재적재 직후 기대하는 **에코**의 원본 지문(08-30 Linux 실기 "같은 항목이 둘").
    /// 플랫폼이 표현을 **일부만** 게시하면(Linux 1단 = 한 표현 · ⇧Enter 평문) 되돌아온
    /// 스냅숏이 원본과 동일하지 않아 새 항목이 된다 — 다음 캡처가 그 원본의 **부분집합**이면
    /// 원본을 승격시키고 새로 넣지 않는다. 한 번 쓰고 비운다.
    pending_echo: Option<u64>,
    /// 다음에 부여할 항목 id(단조 증가 — 복원 시 최댓값+1로 이어진다).
    next_id: u64,
    /// ★ 상한 축출로 빠진 항목 id — 셸이 [`Self::drain_evicted`]로 가져가 저장소에 반영한다.
    evicted: Vec<u64>,
    /// 총용량 예산(바이트 · 0 = 무제한) — 기본 500MB(사용자 확정 09-01).
    max_bytes: u64,
    /// 보관 기간(ms · 0 = 무제한 — 기본값 · 사용자 확정 09-01).
    max_age_ms: u64,
}

/// 정렬된 (이름, 바이트) 위의 FNV-1a 64 — 암호학적일 필요가 없다(후보 거르기 전용,
/// 확정은 바이트 비교).
fn fingerprint(reps: &[RawRep]) -> u64 {
    let mut sorted: Vec<(&str, &[u8])> = reps
        .iter()
        .filter(|r| !is_volatile_format(&r.format))
        .map(|r| (r.format.as_str(), r.data.as_slice()))
        .collect();
    sorted.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x1_0000_01b3);
        }
        h ^= 0xff; // 필드 경계 — "ab"+"c"와 "a"+"bc"를 구분한다.
        h = h.wrapping_mul(0x1_0000_01b3);
    };
    for (f, d) in sorted {
        eat(f.as_bytes());
        eat(d);
    }
    h
}

/// `sub`의 표현 전부가 `sup`에 같은 이름·같은 바이트로 있는가(에코 판정 — 게시한 것만 돌아온다).
fn is_subset(sub: &[RawRep], sup: &[RawRep]) -> bool {
    let core: Vec<&RawRep> = sub
        .iter()
        .filter(|r| !is_volatile_format(&r.format))
        .collect();
    !core.is_empty()
        && core
            .iter()
            .all(|r| sup.iter().any(|s| s.format == r.format && s.data == r.data))
}

fn same_content(a: &[RawRep], b: &[RawRep]) -> bool {
    // 휘발 표현을 뻐고 비교 — 개수도 같은 기준으로 재다.
    let sorted = |s: &[RawRep]| -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = s
            .iter()
            .filter(|r| !is_volatile_format(&r.format))
            .map(|r| (r.format.clone(), r.data.clone()))
            .collect();
        v.sort();
        v
    };
    sorted(a) == sorted(b)
}

impl History {
    /// 상한은 1 이상으로 강제한다(0이면 이력이라는 개념 자체가 사라진다).
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            items: VecDeque::new(),
            cap: cap.max(1),
            pending_echo: None,
            next_id: 1,
            evicted: Vec::new(),
            max_bytes: 0,
            max_age_ms: 0,
        }
    }

    /// ★ 저장소에서 복원 — `items`는 최신이 앞. id 부여는 저장된 최댓값 다음부터 잇는다.
    #[must_use]
    pub fn from_items(cap: usize, items: Vec<HistoryItem>) -> Self {
        let next_id = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        let mut h = Self {
            items: items.into(),
            cap: cap.max(1),
            pending_echo: None,
            next_id,
            evicted: Vec::new(),
            max_bytes: 0,
            max_age_ms: 0,
        };
        h.evict_over_cap();
        h
    }

    /// 상한 초과분을 **오래된 비고정부터** 걷어낸다 — 핀은 지켜진다(설정 문구의 계약).
    /// 걷힌 id는 [`Self::drain_evicted`]가 가져갈 때까지 쌓인다.
    fn evict_over_cap(&mut self) {
        while self.items.len() > self.cap {
            let Some(i) = self.items.iter().rposition(|it| !it.pinned) else {
                break; // 전부 핀 — 상한보다 핀이 우선이다(지우면 계약 위반)
            };
            if let Some(it) = self.items.remove(i) {
                self.evicted.push(it.id);
            }
        }
    }

    /// 상한 축출로 빠진 항목 id를 가져간다(한 번 주면 비운다).
    pub fn drain_evicted(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.evicted)
    }

    /// id로 찾기(S2 메인창 — 필터된 뷰가 인덱스 대신 id로 말한다).
    #[must_use]
    pub fn get_by_id(&self, id: u64) -> Option<&HistoryItem> {
        self.items.iter().find(|it| it.id == id)
    }

    /// ★ 항목 삭제(S2 메인창 — T-18b0). 있었으면 `true`(저장소 동기화는 호출자 몫).
    pub fn remove(&mut self, id: u64) -> bool {
        match self.items.iter().position(|it| it.id == id) {
            Some(i) => {
                self.items.remove(i);
                true
            }
            None => false,
        }
    }

    /// ★ 핀 토글(T-18b0 기초) — 축출에서 지켜진다. 있는 id면 `true`.
    pub fn set_pinned(&mut self, id: u64, pinned: bool) -> bool {
        match self.items.iter_mut().find(|it| it.id == id) {
            Some(it) => {
                it.pinned = pinned;
                true
            }
            None => false,
        }
    }

    /// 재적재 직후 호출 — 다음 캡처가 이 항목의 부분집합이면 새 항목이 아니라 **이 항목의
    /// 승격**으로 처리한다(`i` = 현재 인덱스 · 지문으로 기억하므로 순서가 바뀌어도 맞는다).
    pub fn expect_echo(&mut self, i: usize) {
        self.pending_echo = self.items.get(i).map(|it| it.fingerprint);
    }

    /// 상한 변경(설정 즉시 적용) — 넘치는 꼬리는 지금 잘린다(핀 제외).
    pub fn set_cap(&mut self, cap: usize) {
        self.cap = cap.max(1);
        self.evict_over_cap();
    }

    /// ★ 보관 예산(T-13 · 사용자 확정 09-01: 기본 기간 무제한 + 500MB) —
    /// 총용량 초과·기한 경과분을 **오래된 비고정부터** 걷어낸다(핀 면제 · 구본 created 0 = 기간 면제).
    pub fn set_budget(&mut self, max_bytes: u64, max_age_ms: u64, now_ms: u64) {
        self.max_bytes = max_bytes;
        self.max_age_ms = max_age_ms;
        self.evict_over_budget(now_ms);
    }

    fn evict_over_budget(&mut self, now_ms: u64) {
        // 기간 — 뒤(오래된 쪽)에서부터 기한 경과를 걷는다.
        if self.max_age_ms > 0 {
            while let Some(i) = self
                .items
                .iter()
                .rposition(|it| !it.pinned && it.created_ms > 0)
            {
                if now_ms.saturating_sub(self.items[i].created_ms) <= self.max_age_ms {
                    break; // 가장 오래된 것이 기한 안 = 나머지도 안
                }
                if let Some(it) = self.items.remove(i) {
                    self.evicted.push(it.id);
                }
            }
        }
        // 총용량 — 핀 포함 합계가 예산을 넘으면 오래된 비고정부터.
        if self.max_bytes > 0 {
            let mut total: u64 = self.items.iter().map(|it| it.bytes).sum();
            while total > self.max_bytes {
                let Some(i) = self.items.iter().rposition(|it| !it.pinned) else {
                    break; // 전부 핀 — 예산보다 핀이 우선
                };
                if let Some(it) = self.items.remove(i) {
                    total -= it.bytes;
                    self.evicted.push(it.id);
                }
            }
        }
    }

    /// 스냅숏 하나를 이력에 반영한다. 내용 없음 걸러내기(민감·제외 앱 포함)와
    /// 썸네일 생성은 **호출자 몫**이다 — 이력은 정책도 디코더도 모른다.
    pub fn push(
        &mut self,
        snap: &ClipSnapshot,
        kind: ClipKind,
        label: String,
        thumb: Option<(u32, u32, Vec<u8>)>,
    ) -> Pushed {
        // ① 맨 위의 다음 장면인가(재게시 · 부분→완본) — 교체.
        if let Some(front) = self.items.front() {
            if coalesces_parts(
                front.source_app.as_deref(),
                &front.reps,
                snap.source_app.as_deref(),
                &snap.reps,
            ) {
                // ★ 교체는 같은 항목의 다음 장면 — id·핀을 지킨다(저장소도 같은 id로 덮는다).
                let (id, pinned, copies, created_ms) =
                    (front.id, front.pinned, front.copies, front.created_ms);
                self.items[0] = HistoryItem {
                    id,
                    pinned,
                    kind,
                    label,
                    fingerprint: fingerprint(&snap.reps),
                    bytes: reps_bytes(&snap.reps),
                    reps: snap.reps.clone(),
                    source_app: snap.source_app.clone(),
                    copies,
                    thumb,
                    created_ms,
                };
                return Pushed::Replaced;
            }
        }
        // ①b 재적재 에코(부분 게시) — 기대한 원본의 부분집합이면 원본 승격(한 번만).
        if let Some(src) = self.pending_echo.take() {
            if let Some(i) = self
                .items
                .iter()
                .position(|it| it.fingerprint == src && is_subset(&snap.reps, &it.reps))
            {
                let mut it = self.items.remove(i).unwrap_or_else(|| unreachable!());
                it.copies += 1;
                self.items.push_front(it);
                return Pushed::Promoted;
            }
        }
        // ② 같은 내용이 어딘가 있는가 — 승격(지문으로 거르고 바이트로 확정).
        let fp = fingerprint(&snap.reps);
        if let Some(i) = self
            .items
            .iter()
            .position(|it| it.fingerprint == fp && same_content(&it.reps, &snap.reps))
        {
            let mut it = self.items.remove(i).unwrap_or_else(|| unreachable!());
            it.copies += 1;
            // 승격은 기존 썸네일 유지 — 새로 왔으면(설정을 켠 뒤 재복사 등) 채운다.
            if thumb.is_some() {
                it.thumb = thumb;
            }
            self.items.push_front(it);
            return Pushed::Promoted;
        }
        // ③ 새 항목.
        let id = self.next_id;
        self.next_id += 1;
        let now_ms = epoch_ms();
        self.items.push_front(HistoryItem {
            id,
            pinned: false,
            kind,
            label,
            fingerprint: fp,
            bytes: reps_bytes(&snap.reps),
            reps: snap.reps.clone(),
            source_app: snap.source_app.clone(),
            copies: 1,
            thumb,
            created_ms: now_ms,
        });
        self.evict_over_cap();
        self.evict_over_budget(now_ms);
        Pushed::New
    }

    /// 항목 수.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 비었는가.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 최신부터 `n`개의 라벨(트레이 메뉴용).
    #[must_use]
    pub fn recent_labels(&self, n: usize) -> Vec<String> {
        self.items.iter().take(n).map(|i| i.label.clone()).collect()
    }

    /// 인덱스 접근(0 = 최신) — 재적재용.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&HistoryItem> {
        self.items.get(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(app: &str, reps: &[(&str, &[u8])]) -> ClipSnapshot {
        ClipSnapshot {
            reps: reps
                .iter()
                .map(|(f, d)| RawRep {
                    format: (*f).to_string(),
                    data: d.to_vec(),
                })
                .collect(),
            source_app: Some(app.into()),
            ..Default::default()
        }
    }

    /// ★ 재적재 에코 — 원본(html+plain)을 평문 한 표현만 게시했을 때 되돌아온 스냅숏은
    /// 원본의 부분집합 → 새 항목이 아니라 **원본 승격**(08-30 Linux 실기 "같은 항목 둘").
    /// 기대를 걸지 않았거나 부분집합이 아니면 종전대로 새 항목이다.
    #[test]
    fn partial_repost_echo_promotes_source() {
        let mut h = History::new(10);
        let rich = snap("App", &[("text/html", b"<b>x</b>"), ("text/plain", b"x")]);
        let other = snap("App", &[("text/plain", b"y")]);
        push(&mut h, &rich, "x");
        push(&mut h, &other, "y");
        assert_eq!(h.len(), 2);
        // 재적재(평문만) → 에코.
        h.expect_echo(1);
        let echo = snap("App", &[("text/plain", b"x")]);
        assert_eq!(push(&mut h, &echo, "x"), Pushed::Promoted);
        assert_eq!(h.len(), 2);
        assert_eq!(
            h.get(0).map(|i| i.reps.len()),
            Some(2),
            "원본 표현이 유지된다"
        );
        assert_eq!(h.get(0).map(|i| i.copies), Some(2));
        // 기대는 한 번뿐 — 같은 평문이 또 오면 이제 같은 내용이 없으니 새 항목.
        assert_eq!(push(&mut h, &echo, "x"), Pushed::New);
        // 기대와 무관한 내용은 부분집합이 아니면 새 항목.
        h.expect_echo(0);
        let z = snap("App", &[("text/plain", b"z")]);
        assert_eq!(push(&mut h, &z, "z"), Pushed::New);
    }

    fn push(h: &mut History, s: &ClipSnapshot, label: &str) -> Pushed {
        h.push(s, ClipKind::Text, label.into(), None)
    }

    /// ★ 같은 내용 재복사 = 승격(횟수 +1) — 목록이 도배되지 않는다.
    #[test]
    fn recopy_promotes_instead_of_duplicating() {
        let mut h = History::new(10);
        let a = snap("Code", &[("CF_UNICODETEXT", b"alpha")]);
        let b = snap("Code", &[("CF_UNICODETEXT", b"beta")]);
        assert_eq!(push(&mut h, &a, "alpha"), Pushed::New);
        assert_eq!(push(&mut h, &b, "beta"), Pushed::New);
        // ⚠️ 사이에 다른 복사가 끼면 coalesces(교체)가 아니라 **승격** 경로여야 한다.
        assert_eq!(push(&mut h, &a, "alpha"), Pushed::Promoted);
        assert_eq!(h.len(), 2, "항목이 늘면 안 된다");
        assert_eq!(h.get(0).unwrap().label, "alpha", "승격 = 맨 위로");
        assert_eq!(h.get(0).unwrap().copies, 2, "횟수가 는다");
    }

    /// 같은 첫 줄이라도 **내용이 다르면 다른 항목**이다(라벨 비교의 함정).
    #[test]
    fn same_label_different_content_are_distinct() {
        let mut h = History::new(10);
        let a = snap("Code", &[("CF_UNICODETEXT", b"line\nAAA")]);
        let b = snap("Code", &[("CF_UNICODETEXT", b"line\nBBB")]);
        assert_eq!(push(&mut h, &a, "line"), Pushed::New);
        assert_eq!(push(&mut h, &b, "line"), Pushed::New);
        assert_eq!(h.len(), 2);
    }

    /// ★ 탐색기 재게시·플러시 완본은 **교체**(D-80) — 항목도 횟수도 늘지 않는다.
    #[test]
    fn next_scene_replaces_front() {
        let mut h = History::new(10);
        let partial = snap("explorer", &[("Shell IDList Array", b"x")]);
        let full = snap(
            "explorer",
            &[("Shell IDList Array", b"x"), ("CF_HDROP", b"paths")],
        );
        assert_eq!(push(&mut h, &partial, "부분"), Pushed::New);
        assert_eq!(push(&mut h, &full, "완본"), Pushed::Replaced);
        assert_eq!(h.len(), 1);
        assert_eq!(h.get(0).unwrap().label, "완본");
        assert_eq!(h.get(0).unwrap().copies, 1, "교체는 재복사가 아니다");
        assert_eq!(h.get(0).unwrap().reps.len(), 2, "완본 표현으로 바뀌었다");
    }

    /// 상한 — 오래된 것부터 잘린다. 줄이면 즉시 잘린다.
    #[test]
    fn cap_evicts_oldest() {
        let mut h = History::new(2);
        push(&mut h, &snap("a", &[("CF_UNICODETEXT", b"1")]), "1");
        push(&mut h, &snap("a", &[("CF_UNICODETEXT", b"2")]), "2");
        push(&mut h, &snap("a", &[("CF_UNICODETEXT", b"3")]), "3");
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent_labels(9), ["3", "2"], "가장 오래된 1이 밀려났다");
        h.set_cap(1);
        assert_eq!(h.recent_labels(9), ["3"]);
    }

    /// 지문 경계 — 표현을 다르게 쪼갠 같은 바이트가 같다고 나오면 안 된다.
    #[test]
    fn fingerprint_separates_field_boundaries() {
        let a = snap("x", &[("A", b"ab"), ("B", b"c")]);
        let b = snap("x", &[("A", b"a"), ("B", b"bc")]);
        assert_ne!(fingerprint(&a.reps), fingerprint(&b.reps));
    }
}

#[cfg(test)]
mod persist_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn snap(reps: &[(&str, &[u8])]) -> ClipSnapshot {
        ClipSnapshot {
            reps: reps
                .iter()
                .map(|(f, d)| RawRep {
                    format: (*f).to_string(),
                    data: d.to_vec(),
                })
                .collect(),
            source_app: Some("t".into()),
            concealed: false,
            seq: 0,
        }
    }

    /// id는 단조 증가하고, 복원하면 저장된 최댓값 다음부터 잇는다.
    #[test]
    fn ids_are_monotonic_and_continue_after_restore() {
        let mut h = History::new(10);
        h.push(&snap(&[("T", b"a")]), ClipKind::Text, "a".into(), None);
        h.push(&snap(&[("T", b"b")]), ClipKind::Text, "b".into(), None);
        assert_eq!(h.get(0).unwrap().id, 2);
        assert_eq!(h.get(1).unwrap().id, 1);

        let items: Vec<HistoryItem> = (0..2).map(|i| h.get(i).unwrap().clone()).collect();
        let mut h2 = History::from_items(10, items);
        h2.push(&snap(&[("T", b"c")]), ClipKind::Text, "c".into(), None);
        assert_eq!(h2.get(0).unwrap().id, 3, "최댓값 2 다음 = 3");
    }

    /// ★ 상한 축출은 오래된 **비고정**부터 — 핀은 상한보다 우선이다.
    #[test]
    fn eviction_skips_pinned_and_reports_ids() {
        let mut h = History::new(2);
        h.push(&snap(&[("T", b"a")]), ClipKind::Text, "a".into(), None); // id 1
        h.push(&snap(&[("T", b"b")]), ClipKind::Text, "b".into(), None); // id 2
                                                                         // 가장 오래된 a(id 1)를 핀.
        assert_eq!(h.get(1).unwrap().id, 1, "a는 인덱스 1");
        assert!(h.set_pinned(1, true));
        h.push(&snap(&[("T", b"c")]), ClipKind::Text, "c".into(), None); // id 3 — b가 밀린다
        let labels: Vec<_> = (0..h.len())
            .map(|i| h.get(i).unwrap().label.clone())
            .collect();
        assert_eq!(labels, vec!["c", "a"], "핀(a)은 남고 b가 빠졌다");
        assert_eq!(h.drain_evicted(), vec![2], "빠진 id를 셸이 가져간다");
        assert!(h.drain_evicted().is_empty(), "한 번 주면 비운다");
    }

    /// 교체(coalesce 완본)는 id를 지킨다 — 저장소가 같은 자리를 덮을 수 있게.
    #[test]
    fn replace_keeps_id() {
        let mut h = History::new(10);
        h.push(&snap(&[("T", b"part")]), ClipKind::Text, "p".into(), None);
        let id = h.get(0).unwrap().id;
        // 같은 출처의 상위집합 = 다음 장면(coalesce).
        h.push(
            &snap(&[("T", b"part"), ("H", b"whole")]),
            ClipKind::RichText,
            "w".into(),
            None,
        );
        assert_eq!(h.len(), 1);
        assert_eq!(h.get(0).unwrap().id, id, "교체 후에도 같은 id");
    }
}

#[cfg(test)]
mod budget_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn snap(reps: &[(&str, &[u8])]) -> ClipSnapshot {
        ClipSnapshot {
            reps: reps
                .iter()
                .map(|(f, d)| RawRep {
                    format: (*f).to_string(),
                    data: d.to_vec(),
                })
                .collect(),
            source_app: Some("t".into()),
            concealed: false,
            seq: 0,
        }
    }

    /// ★ 총용량 예산 — 초과하면 오래된 비고정부터 빠지고, 핀은 남는다(09-01 확정).
    #[test]
    fn byte_budget_evicts_oldest_unpinned_keeps_pinned() {
        let mut h = History::new(100);
        let big = vec![0u8; 400];
        h.push(&snap(&[("T", &big)]), ClipKind::Text, "a".into(), None); // id 1
        h.push(&snap(&[("U", &big)]), ClipKind::Text, "b".into(), None); // id 2
        h.push(&snap(&[("V", &big)]), ClipKind::Text, "c".into(), None); // id 3
        assert!(h.set_pinned(1, true));
        h.set_budget(1000, 0, 0); // 1200B > 1000B — 비고정 중 가장 오래된 b(id 2)가 빠진다
        assert_eq!(h.drain_evicted(), vec![2]);
        assert_eq!(h.len(), 2, "핀(a)과 최신(c)이 남는다");
    }

    /// ★ 기간 예산 — 기한 경과 비고정만 · created 0(구본)은 면제.
    #[test]
    fn age_budget_spares_legacy_and_pinned() {
        let now = 10_000_000u64;
        let mk = |id: u64, created: u64, pinned: bool| {
            HistoryItem::restored(
                id,
                ClipKind::Text,
                format!("i{id}"),
                vec![RawRep {
                    format: "T".into(),
                    data: vec![id as u8],
                }],
                None,
                1,
                pinned,
                None,
                created,
            )
        };
        // 최신이 앞: [신품, 구본(0), 핀 낡음, 낡음]
        let items = vec![
            mk(4, now - 10, false),
            mk(3, 0, false),
            mk(2, now - 9_000_000, true),
            mk(1, now - 9_000_000, false),
        ];
        let mut h = History::from_items(100, items);
        h.set_budget(0, 1_000_000, now); // 기한 1_000_000ms
        assert_eq!(h.drain_evicted(), vec![1], "낡은 비고정만");
        assert_eq!(h.len(), 3);
    }

    /// ★ 휘발 벤더 토큰(`ole.source.*`)은 동일성에서 빠진다 — 재복사가 승격이 된다(08-29 관찰).
    #[test]
    fn volatile_vendor_token_does_not_break_promotion() {
        let mut h = History::new(10);
        h.push(
            &snap(&[("HTML", b"x"), ("ole.source.0xAAAA", b"1")]),
            ClipKind::RichText,
            "x".into(),
            None,
        );
        let r = h.push(
            &snap(&[("HTML", b"x"), ("ole.source.0xBBBB", b"2")]),
            ClipKind::RichText,
            "x".into(),
            None,
        );
        assert_eq!(r, Pushed::Promoted, "토큰만 다른 재복사 = 승격");
        assert_eq!(h.len(), 1);
    }
}
