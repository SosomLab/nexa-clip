//! ★ 중복 제외 보기(09-04 사용자) — 메인창·팝업이 **같은 규칙**으로 같은 내용을 한 행으로 합친다.
//!
//! - 내용 열쇠: 텍스트 = 평문(CR 제거 · 끝 공백 제거) · 이미지/개체 = PNG 바이트(없으면 DIB) · 그 외 = 이력 지문.
//! - 대표 행: ★ **핀이 있으면 핀**(10-04) → ★ **서식 있는 글**(10-04) → 로컬 출처가 있으면 로컬(가장 최근) → 가장 최근 수신. 순서는 입력(최신순) 유지.
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
    /// ★ 서식 있는 글인가(`ClipKind::RichText`) — 같은 글의 평문 항목보다 대표로 먼저 선다(10-04).
    pub rich: bool,
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

/// ★ 전파용 **내용 열쇠**(10-10 사용자 — "수신받은 내용은 전달되지 않도록") — 표현 이름·OS 철자와 무관하게
/// "같은 내용"을 가른다: 글 = 평문(CR 제거 · 끝 공백 제거) · 그림/개체 = PNG(없으면 DIB) 바이트 · 파일 = 경로 목록.
/// 이력 항목과 캡처 스냅숏 **양쪽에 같은 함수**를 써서 받은 것과 되읽은 것이 같은 열쇠가 되게 한다.
/// `None` = 내용을 뽑을 수 없음(비교 불가 → 가드는 통과시킨다).
pub(crate) fn payload_key(
    kind: ClipKind,
    reps: &[nclip_core::RawRep],
    plain: Option<&str>,
) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    match kind {
        ClipKind::Image | ClipKind::Object => {
            let r = reps
                .iter()
                .find(|r| {
                    matches!(r.format.as_str(), "PNG" | "public.png" | "image/png")
                        && !r.data.is_empty()
                })
                .or_else(|| {
                    reps.iter().find(|r| {
                        matches!(r.format.as_str(), "CF_DIB" | "CF_DIBV5" | "image/bmp")
                            && !r.data.is_empty()
                    })
                })?;
            "img".hash(&mut h);
            r.data.hash(&mut h);
        }
        ClipKind::Files => {
            let paths = nclip_core::paths_of(reps);
            if paths.is_empty() {
                return None;
            }
            "files".hash(&mut h);
            paths.hash(&mut h);
        }
        _ => {
            let t = plain?;
            let norm = t.replace('\r', "");
            let norm = norm.trim_end();
            if norm.is_empty() {
                return None;
            }
            "txt".hash(&mut h);
            norm.hash(&mut h);
        }
    }
    Some(h.finish())
}

/// ★ 받은 항목이 **지금 맨 앞 항목의 평문판**인가(10-04) — 그렇다면 클립보드에 게시하지 않는다.
///
/// 이 PC에서 서식 글을 복사하면 상대 기기(또는 가상 머신 클립보드 다리를 거친 상대)가 같은 글을
/// **평문만으로** 되돌려 보낼 수 있다. 그것을 게시하면 클립보드의 서식이 평문으로 덮인다.
/// 이력에는 그대로 들어간다(중복 제외 보기가 서식 항목을 대표로 합친다).
pub(crate) fn is_plain_downgrade(
    front_kind: ClipKind,
    front_plain: Option<&str>,
    received_kind: ClipKind,
    received_plain: Option<&str>,
) -> bool {
    let norm = |t: &str| t.replace('\r', "").trim_end().to_string();
    front_kind == ClipKind::RichText
        && received_kind == ClipKind::Text
        && matches!(
            (front_plain, received_plain),
            (Some(a), Some(b)) if !a.trim().is_empty() && norm(a) == norm(b)
        )
}

/// 대표 순위 — 핀 > 서식 > 로컬(큰 쪽이 대표).
type Rank = (bool, bool, bool);
/// 합치는 중인 한 무리 — (대표 인덱스, 로컬 있음, 출처들, 복사 수 합, 대표의 순위).
type Group = (usize, bool, Vec<String>, u32, Rank);

/// 같은 열쇠끼리 합친다 — 남는 행(입력 순서)과 그 메타.
pub(crate) fn merge(entries: &[Entry]) -> Vec<Kept> {
    // key → (대표 인덱스, 로컬 있음, 출처들, 복사 수 합, 대표의 순위)
    let mut groups: HashMap<u64, Group> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let local = !e.remote;
        // 대표 순위 — 핀 > 서식 > 로컬. 같으면 먼저 온 것(= 더 최근).
        //   ★ 핀이 대표를 지킨다(10-04 사용자 실기 — "고정한 항목이 고정 구획에서 사라진다").
        //   종전에는 "수신보다 로컬"만 봐서, **받은 항목을 고정**해 둔 뒤 같은 내용을 이 PC에서
        //   복사하면 대표가 핀 없는 로컬 행으로 넘어가 고정 행이 숨었다.
        //   ★ 서식이 평문을 이긴다(10-04 사용자 실기 — "서식 글을 복사했는데 팝업에서 고르면 평문") —
        //   가상 머신 클립보드 다리(VMware `vmware-user`)는 복사 직후 클립보드를 **평문만으로** 다시
        //   쥔다. 같은 글의 평문 항목이 더 최근 것으로 생겨 대표가 되면 서식 항목이 숨는다.
        let rank = (e.pinned, e.rich, local);
        let g = groups
            .entry(e.key)
            .or_insert_with(|| (i, local, Vec::new(), 0, rank));
        if rank > g.4 {
            g.0 = i;
            g.4 = rank;
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
            rich: false,
            origin: Some(origin.to_string()),
            copies,
        }
    }

    /// ★ 같은 글의 **서식 항목**과 평문 항목 — 평문이 더 최근이어도 대표는 서식 항목이다(10-04 실기:
    /// VMware 클립보드 다리가 복사 직후 평문판을 새로 만들어, 팝업에서 고르면 평문이 붙었다).
    #[test]
    fn rich_row_is_representative_over_newer_plain_duplicate() {
        let mut rich = e(3, false, "ONLYOFFICE", 1);
        rich.rich = true;
        // 입력 0 = 더 최근의 평문(로컬), 입력 1 = 서식.
        let k = merge(&[e(3, false, "vmware-user", 1), rich]);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].keep, 1, "서식 행이 대표");
        assert_eq!(k[0].copies, 2);

        // 수신한 서식 항목도 로컬 평문보다 먼저다 — 단 "내 것" 표시는 로컬 출처가 있으면 유지.
        let mut rich = e(4, true, "⇄ mac", 1);
        rich.rich = true;
        let k = merge(&[e(4, false, "Term", 1), rich]);
        assert_eq!(k[0].keep, 1);
        assert!(!k[0].remote);

        // 핀은 서식보다 먼저다.
        let mut pin = e(5, false, "Term", 1);
        pin.pinned = true;
        let mut rich = e(5, false, "ONLYOFFICE", 1);
        rich.rich = true;
        let k = merge(&[rich, pin]);
        assert_eq!(k[0].keep, 1, "핀 행이 대표");
    }

    /// ★ 받은 평문이 맨 앞 서식 항목과 같은 글이면 게시하지 않는다 — 끝 공백·CR 차이는 같은 글.
    #[test]
    fn plain_downgrade_is_detected_only_for_same_text() {
        use ClipKind::{RichText, Text};
        assert!(is_plain_downgrade(
            RichText,
            Some("a b\r\n"),
            Text,
            Some("a b\n")
        ));
        assert!(!is_plain_downgrade(
            RichText,
            Some("a b"),
            Text,
            Some("a c")
        ));
        assert!(
            !is_plain_downgrade(Text, Some("a b"), Text, Some("a b")),
            "맨 앞이 평문"
        );
        assert!(
            !is_plain_downgrade(RichText, Some("a b"), RichText, Some("a b")),
            "받은 것도 서식"
        );
        assert!(!is_plain_downgrade(RichText, None, Text, Some("a")));
        assert!(!is_plain_downgrade(RichText, Some("  "), Text, Some("")));
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

    /// ★ 전파 내용 열쇠(10-10) — Windows CRLF 글 ↔ Linux/mac LF 글 · 끝 개행 차이 · 표현 이름 차이는 같은 열쇠,
    /// 글이 다르면 다른 열쇠 · 그림은 PNG 바이트 · 내용을 못 뽑으면 None.
    #[test]
    fn payload_key_ignores_line_endings_and_format_names() {
        let reps = |f: &str, d: &[u8]| {
            vec![nclip_core::RawRep {
                format: f.into(),
                data: d.to_vec(),
            }]
        };
        let a = payload_key(
            ClipKind::Text,
            &reps("CF_UNICODETEXT", b"x"),
            Some("hello\r\nworld\r\n"),
        );
        let b = payload_key(
            ClipKind::Text,
            &reps("text/plain", b"x"),
            Some("hello\nworld"),
        );
        let c = payload_key(
            ClipKind::RichText,
            &reps("text/html", b"<b>"),
            Some("hello\nworld"),
        );
        assert!(a.is_some() && a == b, "CRLF·끝 개행만 다른 글 = 같은 열쇠");
        assert_eq!(a, c, "서식 여부는 열쇠에 안 들어간다(같은 글)");
        assert_ne!(a, payload_key(ClipKind::Text, &[], Some("hello world")));
        assert_eq!(
            payload_key(ClipKind::Text, &[], Some("  \r\n")),
            None,
            "빈 글은 비교 불가"
        );
        let png1 = payload_key(ClipKind::Image, &reps("image/png", b"\x89PNG1"), None);
        let png2 = payload_key(ClipKind::Image, &reps("PNG", b"\x89PNG1"), None);
        assert!(
            png1.is_some() && png1 == png2,
            "그림 = PNG 바이트(이름 무관)"
        );
        assert_ne!(
            png1,
            payload_key(ClipKind::Image, &reps("PNG", b"\x89PNG2"), None)
        );
        assert_eq!(
            payload_key(ClipKind::Image, &reps("CF_BITMAP", b"x"), None),
            None
        );
    }
}
