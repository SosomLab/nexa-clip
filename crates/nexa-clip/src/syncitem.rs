//! ★ 클립보드 전파 페이로드(09-04 · DR-6 1단) — OS 표현(`CF_*`·`public.*`·`text/*`)을
//! **휴대 형식**으로 바꿔 종단 세션으로 보내고, 받는 쪽은 자기 OS 표현으로 되돌린다.
//! 1단 범위 = 평문 + PNG 이미지. 파일(경로 목록)·리치 텍스트는 후속.
//!
//! 형식(자체 직렬화 · DR-37): `"NCLI"` ‖ ver u8(1) ‖ nreps u8 ‖ [name_len u8 ‖ name ‖ len u32 LE ‖ data]*
//! 이름은 `image/png` → `text/plain` 순으로 **고정**(같은 내용 = 같은 바이트 — 에코 판정이 해시로 선다).

use nclip_core::RawRep;

const MAGIC: &[u8; 4] = b"NCLI";
/// 페이로드 상한(이미지 포함) — 이보다 크면 보내지 않는다(세션은 청크로 나르지만 RAM·시간 예산).
pub(crate) const MAX_PAYLOAD: usize = 32 * 1024 * 1024;
/// 이미지 긴 변 상한 — 원본이 이보다 크면 줄여 보낸다(스크린숏 4K까지는 원본).
const IMG_SIDE_CAP: u32 = 4096;

/// 휴대 표현 묶음 → 바이트.
#[must_use]
pub(crate) fn encode(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut v = Vec::with_capacity(
        8 + parts
            .iter()
            .map(|(n, d)| n.len() + d.len() + 5)
            .sum::<usize>(),
    );
    v.extend_from_slice(MAGIC);
    v.push(1);
    v.push(parts.len().min(255) as u8);
    for (name, data) in parts.iter().take(255) {
        let n = name.as_bytes();
        v.push(n.len().min(255) as u8);
        v.extend_from_slice(&n[..n.len().min(255)]);
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
    }
    v
}

/// 바이트 → 휴대 표현 묶음(형식 위반·미지 버전은 `None`).
#[must_use]
pub(crate) fn decode(b: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    if b.len() < 6 || &b[..4] != MAGIC || b[4] != 1 {
        return None;
    }
    let n = usize::from(b[5]);
    let mut at = 6;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let nl = usize::from(*b.get(at)?);
        at += 1;
        let name = std::str::from_utf8(b.get(at..at + nl)?).ok()?.to_string();
        at += nl;
        let len = u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize;
        at += 4;
        let data = b.get(at..at + len)?.to_vec();
        at += len;
        out.push((name, data));
    }
    Some(out)
}

/// 페이로드 지문(에코 판정 — 같은 프로세스 안에서만 비교하므로 SipHash 기본 키로 충분).
#[must_use]
pub(crate) fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

fn is_png(fmt: &str) -> bool {
    matches!(fmt, "PNG" | "public.png" | "image/png")
}

/// 이미지 표현 → PNG 바이트(PNG가 있으면 그대로 · DIB/BMP는 디코드 후 인코드 · 긴 변 상한).
fn png_of(reps: &[RawRep]) -> Option<Vec<u8>> {
    if let Some(r) = reps
        .iter()
        .find(|r| is_png(&r.format) && !r.data.is_empty())
    {
        return Some(r.data.clone());
    }
    for i in nclip_core::capture::thumbnail_sources(reps) {
        let r = &reps[i];
        let rgba = match r.format.as_str() {
            "CF_DIB" | "CF_DIBV5" => nclip_core::img::dib_to_rgba(&r.data),
            "image/bmp" if r.data.len() > 14 => nclip_core::img::dib_to_rgba(&r.data[14..]),
            _ => None,
        };
        let Some((w, h, rgba)) = rgba else {
            continue;
        };
        let (w, h, rgba) = if w > IMG_SIDE_CAP || h > IMG_SIDE_CAP {
            nclip_core::img::downscale_rgba(w, h, &rgba, IMG_SIDE_CAP)?
        } else {
            (w, h, rgba)
        };
        if let Some(png) = nclip_plat::imgdec::encode_raw_isolated(w, h, &rgba) {
            return Some(png);
        }
    }
    None
}

/// 캡처/이력 표현 → 휴대 페이로드. 보낼 게 없으면(파일·빈 항목·상한 초과) `None`.
#[must_use]
pub(crate) fn from_reps(reps: &[RawRep]) -> Option<Vec<u8>> {
    use nclip_core::ClipKind;
    let formats: Vec<&str> = reps.iter().map(|r| r.format.as_str()).collect();
    let kind = nclip_core::capture::classify(&formats);
    if kind == ClipKind::Files {
        return None; // 경로 목록 전파는 후속(DR-6 "파일은 경로만").
    }
    let png = matches!(kind, ClipKind::Image | ClipKind::Object)
        .then(|| png_of(reps))
        .flatten();
    let text = crate::main_win::plain_of(reps);
    let mut parts: Vec<(&str, &[u8])> = Vec::new();
    if let Some(p) = png.as_deref() {
        parts.push(("image/png", p));
    }
    if let Some(t) = text.as_deref() {
        parts.push(("text/plain", t.as_bytes()));
    }
    if parts.is_empty() {
        return None;
    }
    let out = encode(&parts);
    (out.len() <= MAX_PAYLOAD).then_some(out)
}

/// 휴대 표현 묶음 → **이 OS**의 클립보드 표현(게시·이력 등재 공용).
#[must_use]
pub(crate) fn to_local_reps(parts: &[(String, Vec<u8>)]) -> Vec<RawRep> {
    let mut reps = Vec::new();
    for (name, data) in parts {
        match name.as_str() {
            "image/png" => reps.extend(png_reps(data)),
            "text/plain" => {
                if let Ok(t) = std::str::from_utf8(data) {
                    reps.extend(nclip_plat::clipboard::plain_text_reps(t));
                }
            }
            _ => {} // 미지 표현 — 전방 호환(조용히 버림)
        }
    }
    reps
}

/// PNG → OS 이미지 표현. Windows는 `PNG` + `CF_DIB`(대부분의 앱이 DIB만 읽는다 — "이미지로 복사"와 동일).
fn png_reps(png: &[u8]) -> Vec<RawRep> {
    #[cfg(target_os = "windows")]
    {
        let mut v = vec![RawRep {
            format: "PNG".to_string(),
            data: png.to_vec(),
        }];
        if let Some((w, h, rgba)) = nclip_plat::imgdec::decode_isolated(png, IMG_SIDE_CAP) {
            v.push(RawRep {
                format: "CF_DIB".to_string(),
                data: crate::render_img::dib_from_rgba(w, h, &rgba),
            });
        }
        v
    }
    #[cfg(target_os = "macos")]
    {
        vec![RawRep {
            format: "public.png".to_string(),
            data: png.to_vec(),
        }]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        vec![RawRep {
            format: "image/png".to_string(),
            data: png.to_vec(),
        }]
    }
}

/// 로그용 요약("text 12자" · "image 34KB").
#[must_use]
pub(crate) fn describe(parts: &[(String, Vec<u8>)]) -> String {
    parts
        .iter()
        .map(|(n, d)| match n.as_str() {
            "text/plain" => format!("text {}자", String::from_utf8_lossy(d).chars().count()),
            "image/png" => format!("image {}KB", d.len() / 1024),
            other => format!("{other} {}B", d.len()),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let e = encode(&[("image/png", b"PNG..."), ("text/plain", "안녕".as_bytes())]);
        let d = decode(&e).expect("decode");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].0, "image/png");
        assert_eq!(d[1].1, "안녕".as_bytes());
        assert_eq!(decode(b"NOPE"), None);
        assert_eq!(decode(&e[..10]), None, "잘림 = None");
    }

    #[test]
    fn text_reps_become_portable_text() {
        let reps = nclip_plat::clipboard::plain_text_reps("hello sync");
        let p = from_reps(&reps).expect("payload");
        let d = decode(&p).expect("decode");
        assert_eq!(d, vec![("text/plain".to_string(), b"hello sync".to_vec())]);
        // 되돌리면 이 OS 평문 표현이 나오고, 다시 휴대형으로 만들면 같은 바이트(에코 해시 근거).
        let back = to_local_reps(&d);
        assert_eq!(from_reps(&back).as_deref(), Some(p.as_slice()));
    }

    #[test]
    fn files_are_not_sent() {
        let reps = vec![RawRep {
            format: "CF_HDROP".to_string(),
            data: vec![0; 40],
        }];
        assert!(from_reps(&reps).is_none());
    }
}
