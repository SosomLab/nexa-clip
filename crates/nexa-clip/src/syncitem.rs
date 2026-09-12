//! ★ 클립보드 전파 페이로드(09-04 · DR-6 1단) — OS 표현(`CF_*`·`public.*`·`text/*`)을
//! **휴대 형식**으로 바꿔 종단 세션으로 보내고, 받는 쪽은 자기 OS 표현으로 되돌린다.
//! 범위 = 평문 · PNG 이미지 · ★ **파일 경로 목록**(09-12 · DR-6 "파일은 내용이 아니라 목록").
//! 리치 텍스트는 후속.
//!
//! 형식(자체 직렬화 · DR-37): `"NCLI"` ‖ ver u8(1) ‖ nreps u8 ‖ [name_len u8 ‖ name ‖ len u32 LE ‖ data]*
//! 이름 순서는 **고정**이다 — `x-nclip/paths` → `image/png` → `text/plain`
//! (같은 내용 = 같은 바이트 — 에코 판정이 해시로 선다).
//!
//! ## ★ 파일은 "보내는 쪽"보다 **받는 쪽**이 어렵다([docs/08 §3](../../../docs/08-clipboard-propagation.md))
//!
//! `D:\작업\보고서.xlsx`를 보내면 상대 PC에는 그 경로가 **없다**. 그래서 받는 쪽이
//! 붙여넣기 직전에 실재를 확인해 갈라 놓는다(적응형 · 안 "다"):
//! **전부 실재 → 파일 객체**(P-2) · **하나라도 부재 → 경로 텍스트**(P-3) ·
//! 어느 쪽이었는지 **항상 로그로 남긴다**(P-4 — 조용히 실패하지 않는다).
//! NAS·같은 경로 구조를 쓰는 환경에서는 이것이 그대로 완전한 파일 전파처럼 동작한다.

use nclip_core::RawRep;

const MAGIC: &[u8; 4] = b"NCLI";
/// ★ 파일 경로 목록 파트 — **NUL(`\0`) 구분 원본 경로**(09-12).
///
/// `text/uri-list`가 아니라 **OS 경로 문자열 그대로** 싣는 이유:
/// ① 실재 판정(P-1)은 OS 경로로 해야 하고, ② URI 왕복은 Windows 드라이브 문자·역슬래시에서
/// 손실이 나기 쉬우며, ③ 이 이름을 모르는 **구버전(≤0.1.3)은 조용히 버리고** 함께 실은
/// `text/plain`을 경로 텍스트로 붙인다 — 앞뒤 호환이 공짜로 선다([`decode`]의 미지 표현 규칙).
///
/// 구분자가 `\0`인 것은 **어느 OS에서도 경로에 들어갈 수 없는 유일한 바이트**이기 때문이다
/// (`\n`은 Linux 파일명에 들어갈 수 있어 목록이 조용히 쪼개진다).
const PART_PATHS: &str = "x-nclip/paths";
/// 받은 파일 항목을 **항상 경로 텍스트로** 올린다(설정 `sync.files_paste` = `text`).
/// 세션 스레드가 읽고 UI 스레드(설정 변경·부팅)가 쓴다 — [`set_files_as_text`].
static FILES_AS_TEXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 수신 파일 정책 갱신(부팅·설정 변경 즉시 — [`crate::sync_cmd::set_policy`]와 같은 화법).
pub(crate) fn set_files_as_text(v: bool) {
    FILES_AS_TEXT.store(v, std::sync::atomic::Ordering::Relaxed);
}
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

/// 캡처/이력 표현 → 휴대 페이로드. 보낼 게 없으면(빈 항목·경로 없는 파일·상한 초과) `None`.
///
/// 파일 항목은 **개수 무제한**으로 본다 — 설정 상한을 거는 자리는 송신 한 곳뿐이라
/// ([`from_reps_limited`]) 수신측 에코 지문 계산이 상한에 흔들리지 않는다.
#[must_use]
pub(crate) fn from_reps(reps: &[RawRep]) -> Option<Vec<u8>> {
    from_reps_limited(reps, Some(usize::MAX))
}

/// [`from_reps`] + **파일 경로 정책**(09-12) — `files_max`:
/// `None` = 파일 항목을 보내지 않는다(설정 `sync.files` 끔) · `Some(n)` = 경로 최대 n개.
#[must_use]
pub(crate) fn from_reps_limited(reps: &[RawRep], files_max: Option<usize>) -> Option<Vec<u8>> {
    use nclip_core::ClipKind;
    let formats: Vec<&str> = reps.iter().map(|r| r.format.as_str()).collect();
    let kind = nclip_core::capture::classify(&formats);
    if kind == ClipKind::Files {
        let max = files_max?; // 설정에서 껐다 — 경로 텍스트도 보내지 않는다.
        let mut paths = nclip_core::paths_of(reps);
        if paths.is_empty() {
            // 경로를 못 뽑는 파일 항목 — 탐색기 **잘라내기**는 `CF_HDROP` 없이
            // `Shell IDList Array`(PIDL)만 온다(docs/27 ⑧). 지어내지 않고 보내지 않는다.
            return None;
        }
        if paths.len() > max {
            println!(
                "동기화: 파일 {}개 중 {max}개만 전파합니다(설정 → 동기화 → 파일 경로 최대 개수)",
                paths.len()
            );
            paths.truncate(max);
        }
        // 경로는 NUL 구분 원본, 텍스트는 사람이 읽는 줄 목록 — 둘 다 실어 구버전·텍스트 앱까지 받는다.
        let joined = paths.join("\0");
        let text = paths.join("\r\n");
        let out = encode(&[
            (PART_PATHS, joined.as_bytes()),
            ("text/plain", text.as_bytes()),
        ]);
        return (out.len() <= MAX_PAYLOAD).then_some(out);
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

/// 받은 경로 목록 → **이 OS의 파일 표현**(적응형 · [docs/08 §3-2](../../../docs/08-clipboard-propagation.md)).
///
/// | 상황 | 결과 |
/// |---|---|
/// | 설정이 *경로 텍스트* | 빈 목록 — 함께 온 `text/plain`이 경로 글자로 붙는다 |
/// | 경로 전부 실재(P-2) | `CF_HDROP`·`NSFilenamesPboardType`·`gnome-copied-files` |
/// | 하나라도 부재(P-3) | 빈 목록 + **부재 개수를 로그로**(조용히 실패 금지 · P-4) |
///
/// ⚠️ 실재 확인은 **파일 시스템 접근**이다 — 세션 스레드에서만 부른다(UI는 기다리지 않는다).
fn file_reps_of(data: &[u8]) -> Vec<RawRep> {
    let paths: Vec<String> = data
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .filter_map(|p| std::str::from_utf8(p).ok())
        .map(str::to_string)
        .collect();
    if paths.is_empty() {
        return Vec::new();
    }
    if FILES_AS_TEXT.load(std::sync::atomic::Ordering::Relaxed) {
        println!(
            "동기화: 파일 {}개 — 설정이 '경로 텍스트'라 경로 글자로 올립니다",
            paths.len()
        );
        return Vec::new();
    }
    let missing = paths
        .iter()
        .filter(|p| !std::path::Path::new(p).exists())
        .count();
    if missing > 0 {
        println!(
            "동기화: 파일 {}개 중 {missing}개가 이 기기에 없습니다 — 경로 글자로 올립니다(파일로는 붙여넣을 수 없습니다)",
            paths.len()
        );
        return Vec::new();
    }
    let reps = nclip_plat::clipboard::file_reps(&paths);
    if reps.is_empty() {
        println!(
            "동기화: 파일 {}개 — 이 OS는 파일 표현 게시가 미이식이라 경로 글자로 올립니다",
            paths.len()
        );
    } else {
        println!(
            "동기화: 파일 {}개 — 경로가 전부 실재해 **파일로** 올립니다",
            paths.len()
        );
    }
    reps
}

/// 휴대 표현 묶음 → **이 OS**의 클립보드 표현(게시·이력 등재 공용).
#[must_use]
pub(crate) fn to_local_reps(parts: &[(String, Vec<u8>)]) -> Vec<RawRep> {
    let mut reps = Vec::new();
    for (name, data) in parts {
        match name.as_str() {
            // ★ 파일 경로(09-12) — 적응형 판정은 **여기**서(세션 스레드 · 파일 실재 확인이
            //   디스크·네트워크 드라이브를 건드리므로 UI 스레드에 두면 멈춘다 · DR-41).
            PART_PATHS => reps.extend(file_reps_of(data)),
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
            PART_PATHS => format!(
                "files {}개",
                d.split(|b| *b == 0).filter(|p| !p.is_empty()).count()
            ),
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

    /// 파일 실재 판정을 건드리는 시험들은 **전역 정책 하나**를 공유한다 — 직렬화한다.
    fn policy_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
        L.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn paths_in(payload: &[u8]) -> Vec<String> {
        let parts = decode(payload).expect("decode");
        assert_eq!(parts[0].0, PART_PATHS, "경로가 첫 파트다(이름 순서 고정)");
        parts[0]
            .1
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect()
    }

    /// ★ 파일 항목은 **경로 목록 + 경로 텍스트** 두 파트로 간다(09-12 · DR-6).
    /// 텍스트를 함께 싣는 것이 구버전(≤0.1.3)·텍스트 앱 폴백의 전부다.
    #[test]
    fn file_item_travels_as_paths_plus_text() {
        let paths = vec!["/srv/보고서 초안.xlsx".to_string(), "/srv/b.md".to_string()];
        let reps = nclip_plat::clipboard::file_reps(&paths);
        let payload = from_reps(&reps).expect("payload");
        assert_eq!(paths_in(&payload), paths);
        let parts = decode(&payload).expect("decode");
        assert_eq!(parts[1].0, "text/plain");
        assert_eq!(
            String::from_utf8(parts[1].1.clone()).expect("경로 텍스트는 UTF-8"),
            paths.join("\r\n")
        );
    }

    /// 설정 상한(`sync.files_max`)은 **보내기 한 곳**에서만 걸린다.
    #[test]
    fn file_paths_are_capped_by_setting() {
        let paths: Vec<String> = (0..5).map(|i| format!("/srv/{i}.txt")).collect();
        let reps = nclip_plat::clipboard::file_reps(&paths);
        let payload = from_reps_limited(&reps, Some(2)).expect("payload");
        assert_eq!(paths_in(&payload).len(), 2);
        // 끄면 경로 텍스트조차 나가지 않는다(사용자가 파일을 공유하지 않기로 한 것이다).
        assert!(from_reps_limited(&reps, None).is_none());
    }

    /// 탐색기 **잘라내기**는 `CF_HDROP` 없이 온다 — 경로를 못 뽑으면 **보내지 않는다**(지어내기 금지).
    #[test]
    fn file_item_without_paths_is_not_sent() {
        let reps = vec![RawRep {
            format: "CF_HDROP".to_string(),
            data: vec![0; 40],
        }];
        assert!(from_reps(&reps).is_none());
    }

    /// ★ 적응형 P-2 — 경로가 실재하면 **파일 표현**으로 되돌린다.
    #[test]
    fn existing_paths_come_back_as_files() {
        let _g = policy_lock();
        set_files_as_text(false);
        let me = std::env::current_exe().expect("exe");
        let path = me.to_string_lossy().into_owned();
        let parts = vec![(PART_PATHS.to_string(), path.clone().into_bytes())];
        let reps = to_local_reps(&parts);
        assert!(!reps.is_empty(), "실재하는 경로는 파일로 올린다");
        assert_eq!(nclip_core::paths_of(&reps), vec![path]);
    }

    /// ★ 적응형 P-3 — 하나라도 없으면 파일 표현을 만들지 않는다(함께 온 텍스트가 붙는다).
    #[test]
    fn missing_paths_make_no_file_reps() {
        let _g = policy_lock();
        set_files_as_text(false);
        let parts = vec![(
            PART_PATHS.to_string(),
            b"/nexa-clip/this/does/not/exist.txt".to_vec(),
        )];
        assert!(to_local_reps(&parts).is_empty());
    }

    /// ★ **실제 클립보드 전 구간 왕복**(09-12 · Windows) — 보내는 쪽 표현 → 페이로드 → 받는 쪽
    /// 적응형 변환 → **OS 클립보드 게시** → 감시 재독 → 경로 동일 · **에코 지문 동일**.
    ///
    /// 마지막 등식이 이 기능의 심장이다: 받아서 게시한 것을 감시가 되읽어 `from_reps`에 넣었을 때
    /// **받은 바이트와 같아야** 되돌려 보내지 않는다(핑퐁 차단의 근거). 사용자 클립보드는
    /// 시작 전 것을 끝에 되돌려 놓는다(최선 노력).
    ///
    /// 실제 클립보드를 만지므로 기본 실행에서는 건너뛴다 — `cargo test -p nexa-clip -- --ignored real_clipboard`.
    #[cfg(windows)]
    #[test]
    #[ignore = "실제 클립보드를 사용(수동 실행 전용)"]
    fn real_clipboard_file_round_trip_keeps_echo_fingerprint() {
        let _g = policy_lock();
        set_files_as_text(false);
        let before = nclip_plat::watch_win::read_snapshot();
        // ① 보내는 쪽 — 실재하는 경로 둘(현재 exe · 그 폴더)로 만든 파일 항목.
        let exe = std::env::current_exe().expect("exe");
        let dir = exe.parent().expect("parent").to_path_buf();
        let paths = vec![
            exe.to_string_lossy().into_owned(),
            dir.to_string_lossy().into_owned(),
        ];
        let sender = nclip_plat::clipboard::file_reps(&paths);
        let payload = from_reps(&sender).expect("payload");
        // ② 받는 쪽 — 디코드 → 적응형(전부 실재) → 파일 표현 + 경로 텍스트.
        let parts = decode(&payload).expect("decode");
        let local = to_local_reps(&parts);
        assert!(
            local.iter().any(|r| r.format == "CF_HDROP"),
            "실재하는 경로는 파일로"
        );
        assert!(
            local.iter().any(|r| r.format == "CF_UNICODETEXT"),
            "텍스트 폴백 동반"
        );
        assert_eq!(
            from_reps(&local).as_deref(),
            Some(payload.as_slice()),
            "세션 스레드가 게시할 표현에서 계산한 지문 = 받은 페이로드"
        );
        // ③ OS 클립보드 게시 → 감시 재독.
        let n = nclip_plat::clipboard::set_reps(&local).expect("게시");
        assert!(n >= 2, "CF_HDROP + 텍스트 이상 게시");
        let snap = nclip_plat::watch_win::read_snapshot().expect("읽기");
        assert_eq!(
            nclip_core::classify(&snap.formats()),
            nclip_core::ClipKind::Files
        );
        assert_eq!(snap.file_paths(), paths, "탐색기가 볼 경로 목록");
        assert_eq!(
            from_reps(&snap.reps).as_deref(),
            Some(payload.as_slice()),
            "감시가 되읽은 표현 → 같은 바이트(에코 흡수)"
        );
        // ④ 사용자 클립보드 복원(최선 노력).
        if let Some(b) = before {
            let _ = nclip_plat::clipboard::set_reps(&b.reps);
        }
    }

    /// 설정이 *경로 텍스트* 면 실재 확인조차 하지 않는다.
    #[test]
    fn text_only_policy_skips_files() {
        let _g = policy_lock();
        set_files_as_text(true);
        let me = std::env::current_exe().expect("exe");
        let parts = vec![(
            PART_PATHS.to_string(),
            me.to_string_lossy().into_owned().into_bytes(),
        )];
        let empty = to_local_reps(&parts).is_empty();
        set_files_as_text(false);
        assert!(empty, "정책이 텍스트면 파일 표현을 만들지 않는다");
    }
}
