//! ★ 중복 제외 보기(09-04 사용자) — 메인창·팝업이 **같은 규칙**으로 같은 내용을 한 행으로 합친다.
//!
//! - 내용 열쇠: 텍스트 = 평문(CR 제거 · 끝 공백 제거) · 이미지/개체 = PNG 바이트(없으면 DIB) · 그 외 = 이력 지문.
//! - 대표 행: ★ **핀이 있으면 핀**(10-04) → 로컬 출처가 있으면 로컬(가장 최근) → 가장 최근 수신. 순서는 입력(최신순) 유지.
//! - 메타: 출처 합집합(로컬 앞 · `⇄ 기기` 뒤) · 복사 수 합 · 로컬 출처가 하나라도 있으면 "내 것"(수신 점 없음).

use nclip_core::history::{History, HistoryItem};
use nclip_core::ClipKind;
use std::collections::HashMap;

/// 합치기 입력 한 줄(창이 자기 행에서 뽑아 준다).
pub(crate) struct Entry {
    pub key: u64,
    pub remote: bool,
    /// ★ 고정 항목인가 — 대표 선택에서 **가장 먼저** 본다(10-04).
    pub pinned: bool,
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
    // key → (대표 인덱스, 로컬 있음, 출처들, 복사 수 합, 대표가 핀)
    let mut groups: HashMap<u64, (usize, bool, Vec<String>, u32, bool)> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let local = !e.remote;
        let g = groups
            .entry(e.key)
            .or_insert_with(|| (i, local, Vec::new(), 0, e.pinned));
        // ★ 핀이 대표를 지킨다(10-04 사용자 실기 — "고정한 항목이 고정 구획에서 사라진다").
        //   종전에는 "수신보다 로컬"만 봐서, **받은 항목을 고정**해 둔 뒤 같은 내용을 이 PC에서
        //   복사하면 대표가 핀 없는 로컬 행으로 넘어가 고정 행이 숨었다.
        if e.pinned && !g.4 {
            g.0 = i;
            g.4 = true;
        } else if local && !g.1 && !g.4 {
            g.0 = i; // 먼저 온 게 수신이고 이건 로컬 — 로컬이 대표
        }
        g.1 |= local;
        if let Some(o) = &e.origin {
            if !g.2.contains(o) {
                g.2.push(o.clone());
            }
        }
        g.3 = g.3.saturating_add(e.copies);
    }
    let mut out: Vec<Kept> = groups
        .into_iter()
        .map(|(_, (keep, has_local, origins, copies, _))| {
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
            pinned: false,
            origin: Some(origin.to_string()),
            copies,
        }
    }

    /// ★ 고정한 **수신** 항목 + 같은 내용의 로컬 항목 — 대표는 핀 행이어야 한다(10-04 실기:
    /// 핀이 고정 구획에서 사라지고 내용만 일반 구획에 남았다). 메타는 그대로 합친다.
    #[test]
    fn pinned_row_stays_representative_over_local_duplicate() {
        let mut pin = e(1, true, "⇄ mac", 1);
        pin.pinned = true;
        // 창은 핀 구획을 먼저 넣는다 — 입력 0 = 핀(수신), 입력 1 = 같은 내용의 로컬.
        let k = merge(&[pin, e(1, false, "Code", 2)]);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].keep, 0, "핀 행이 대표");
        assert_eq!(k[0].origins, vec!["Code".to_string(), "⇄ mac".to_string()]);
        assert_eq!(k[0].copies, 3);
        assert!(!k[0].remote, "로컬 출처가 있으면 수신 점은 없다");

        // 핀이 뒤에 와도(입력 순서와 무관하게) 핀이 대표.
        let mut pin = e(7, false, "Code", 1);
        pin.pinned = true;
        let k = merge(&[e(7, false, "Term", 1), pin]);
        assert_eq!(k[0].keep, 1);
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
