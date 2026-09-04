//! ★ 중복 제외 보기(09-04 사용자) — 메인창·팝업이 **같은 규칙**으로 같은 내용을 한 행으로 합친다.
//!
//! - 내용 열쇠: 텍스트 = 평문(CR 제거 · 끝 공백 제거) · 이미지/개체 = PNG 바이트(없으면 DIB) · 그 외 = 이력 지문.
//! - 대표 행: 로컬 출처가 있으면 로컬(가장 최근), 없으면 가장 최근 수신. 순서는 입력(최신순) 유지.
//! - 메타: 출처 합집합(로컬 앞 · `⇄ 기기` 뒤) · 복사 수 합 · 로컬 출처가 하나라도 있으면 "내 것"(수신 점 없음).

use nclip_core::history::{History, HistoryItem};
use nclip_core::ClipKind;
use std::collections::HashMap;

/// 합치기 입력 한 줄(창이 자기 행에서 뽑아 준다).
pub(crate) struct Entry {
    pub key: u64,
    pub remote: bool,
    pub origin: Option<String>,
    pub copies: u32,
}

/// 합치기 결과 — 입력 인덱스 `keep`의 행을 남기고 메타를 덮어쓴다.
pub(crate) struct Kept {
    pub keep: usize,
    pub origins: Vec<String>,
    pub copies: u32,
    pub remote: bool,
}

/// 원격 수신 출처 표식(이력의 `remote_origin`과 같은 규약).
pub(crate) const REMOTE_MARK: &str = "⇄ ";

/// 내용 열쇠.
pub(crate) fn content_key_of(item: &HistoryItem, plain: Option<&str>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    let image_rep = matches!(item.kind, ClipKind::Image | ClipKind::Object).then(|| {
        item.reps
            .iter()
            .find(|r| {
                matches!(r.format.as_str(), "PNG" | "public.png" | "image/png")
                    && !r.data.is_empty()
            })
            .or_else(|| {
                item.reps.iter().find(|r| {
                    matches!(r.format.as_str(), "CF_DIB" | "CF_DIBV5" | "image/bmp")
                        && !r.data.is_empty()
                })
            })
    });
    match (image_rep.flatten(), plain) {
        (Some(r), _) => {
            "img".hash(&mut h);
            r.data.hash(&mut h);
        }
        (None, Some(t)) if !t.trim().is_empty() => {
            "txt".hash(&mut h);
            t.replace('\r', "").trim_end().hash(&mut h);
        }
        _ => {
            "fp".hash(&mut h);
            History::content_key(item).hash(&mut h);
        }
    }
    h.finish()
}

/// 같은 열쇠끼리 합친다 — 남는 행(입력 순서)과 그 메타.
pub(crate) fn merge(entries: &[Entry]) -> Vec<Kept> {
    // key → (대표 인덱스, 로컬 있음, 출처들, 복사 수 합)
    let mut groups: HashMap<u64, (usize, bool, Vec<String>, u32)> = HashMap::new();
    let mut order: Vec<u64> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let local = !e.remote;
        let g = groups.entry(e.key).or_insert_with(|| {
            order.push(e.key);
            (i, local, Vec::new(), 0)
        });
        if local && !g.1 {
            g.0 = i; // 먼저 온 게 수신이고 이건 로컬 — 로컬이 대표
            g.1 = true;
        }
        if let Some(o) = &e.origin {
            if !g.2.contains(o) {
                g.2.push(o.clone());
            }
        }
        g.3 = g.3.saturating_add(e.copies);
    }
    let mut out: Vec<Kept> = groups
        .into_iter()
        .map(|(_, (keep, has_local, origins, copies))| {
            let mut sorted: Vec<String> = origins
                .iter()
                .filter(|o| !o.starts_with(REMOTE_MARK))
                .cloned()
                .collect();
            sorted.extend(
                origins
                    .iter()
                    .filter(|o| o.starts_with(REMOTE_MARK))
                    .cloned(),
            );
            Kept {
                keep,
                origins: sorted,
                copies,
                remote: !has_local,
            }
        })
        .collect();
    out.sort_by_key(|k| k.keep); // 입력 순서(최신순) 유지
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(key: u64, remote: bool, origin: &str, copies: u32) -> Entry {
        Entry {
            key,
            remote,
            origin: Some(origin.to_string()),
            copies,
        }
    }

    #[test]
    fn local_wins_regardless_of_order_and_meta_merges() {
        // 수신이 먼저, 로컬이 뒤 — 로컬이 대표 · 출처는 로컬 앞 · 복사 수 합.
        let k = merge(&[
            e(1, true, "⇄ mac", 2),
            e(2, true, "⇄ mac", 1),
            e(1, false, "Code", 3),
        ]);
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].keep, 1, "다른 열쇠(2)는 자기 자리");
        assert_eq!(k[1].keep, 2, "열쇠 1은 로컬(입력 2)이 대표");
        assert_eq!(k[1].origins, vec!["Code".to_string(), "⇄ mac".to_string()]);
        assert_eq!(k[1].copies, 5);
        assert!(!k[1].remote);
        assert!(k[0].remote, "수신만 있는 묶음은 수신");
    }
}
