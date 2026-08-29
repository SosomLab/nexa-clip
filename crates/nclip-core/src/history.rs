//! 메모리 이력 — ★ **잡은 것을 들고 있다가 승격·상한을 적용한다**(T-13 1단).
//!
//! 저장소(T-16)가 붙기 전의 **세션 한정** 이력이다. 그래도 규칙은 최종형 그대로다:
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
    /// 내용 지문(빠른 동일성 후보 판정).
    fingerprint: u64,
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
}

/// 정렬된 (이름, 바이트) 위의 FNV-1a 64 — 암호학적일 필요가 없다(후보 거르기 전용,
/// 확정은 바이트 비교).
fn fingerprint(reps: &[RawRep]) -> u64 {
    let mut sorted: Vec<(&str, &[u8])> = reps
        .iter()
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
    !sub.is_empty()
        && sub
            .iter()
            .all(|r| sup.iter().any(|s| s.format == r.format && s.data == r.data))
}

fn same_content(a: &[RawRep], b: &[RawRep]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let sorted = |s: &[RawRep]| -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = s
            .iter()
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
        }
    }

    /// 재적재 직후 호출 — 다음 캡처가 이 항목의 부분집합이면 새 항목이 아니라 **이 항목의
    /// 승격**으로 처리한다(`i` = 현재 인덱스 · 지문으로 기억하므로 순서가 바뀌어도 맞는다).
    pub fn expect_echo(&mut self, i: usize) {
        self.pending_echo = self.items.get(i).map(|it| it.fingerprint);
    }

    /// 상한 변경(설정 즉시 적용) — 넘치는 꼬리는 지금 잘린다.
    pub fn set_cap(&mut self, cap: usize) {
        self.cap = cap.max(1);
        self.items.truncate(self.cap);
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
                let copies = front.copies;
                self.items[0] = HistoryItem {
                    kind,
                    label,
                    fingerprint: fingerprint(&snap.reps),
                    reps: snap.reps.clone(),
                    source_app: snap.source_app.clone(),
                    copies,
                    thumb,
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
        self.items.push_front(HistoryItem {
            kind,
            label,
            fingerprint: fp,
            reps: snap.reps.clone(),
            source_app: snap.source_app.clone(),
            copies: 1,
            thumb,
        });
        self.items.truncate(self.cap);
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
