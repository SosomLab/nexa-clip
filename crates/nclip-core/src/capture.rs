//! 캡처 파이프라인 — ★ **클립보드 한 벌을 항목 하나로 바꾸는 규칙**(T-14a).
//!
//! 설계 원천은 [docs/27](../../../docs/27-capture-cases.md). 여기는 그 문서의
//! **판정 규칙만** 담는다 — OS 호출도, 디코딩도, 파일 접근도 없다(순수 함수).
//! 그래서 세 OS의 실제 클립보드 없이도 **전부 테스트할 수 있다**.
//!
//! ## 하는 일 세 가지
//!
//! | # | 무엇 | 왜 여기 있나 |
//! |:--:|---|---|
//! | ① | [`classify`] — 표현 이름들 → [`ClipKind`] | ★ **순서가 전부다**([§1](#1-종류-판정)) |
//! | ② | [`primary_index`] · [`thumbnail_source`] | 붙여넣기 사다리 꼭대기 · 미리보기 원본 |
//! | ③ | [`select_reps`] — 용량 상한 적용 | Office 한 번 복사가 20MB다 |
//!
//! ## 1. 종류 판정
//!
//! ```text
//! ① 파일 목록 표현이 있다                 → Files
//! ② HTML 또는 RTF가 있다                  → RichText
//! ③ 벤더(앱 고유) 포맷이 있다
//!      ├ 평문이 함께 있다                 → RichText   (Excel 범위 · 글 있는 도형)
//!      └ 아니면                           → Object     ★ PPT 도형 · 차트
//! ④ 비트맵/메타파일이 있다                → Image      (벤더 없는 순수 그림)
//! ⑤ 그 외                                → Text
//! ```
//!
//! ⚠️ **이 순서를 뒤집으면 조용히 틀린다.**
//!
//! | 뒤집으면 | 무슨 일이 나나 |
//! |---|---|
//! | 비트맵을 벤더보다 먼저 | ★ **PPT 도형 복사에도 `CF_DIB`가 들어 있다** — 도형이 "이미지"로 분류되고 검색·필터·보관 정책이 전부 틀어진다 |
//! | 파일을 나중에 | 탐색기 복사에도 파일 이름이 담긴 평문이 붙는다 → 파일 3개가 "텍스트"가 된다 |
//! | 벤더면 무조건 리치 | ★ **글이 하나도 없는 그림도 "서식 있는 글"이 된다**(08-27 실기) |
//! | 글 없는 벤더를 그림으로 | ★ **PPT 도형이 "이미지"가 된다** — 붙여넣으면 편집 가능한 도형인데 그림이라고 말한다(08-27 2차) |
//!
//! ## 2. ★ 벤더 포맷을 **목록으로 알아보지 않는다**
//!
//! `"Art::GVML ClipFormat"` 같은 이름을 표에 적어 두면 **그 표는 영원히 늙는다**.
//! 대신 뒤집어 본다 — **우리가 아는 표준(파일·HTML·RTF·비트맵·평문)이 아니면 벤더다.**
//!
//! 클립보드의 성질이 이 규칙을 정당화한다: **모르는 포맷을 넣은 앱이 그걸 읽을 줄 아는 앱**이다.
//! 우리는 이름째로 보관했다가 그대로 돌려주기만 하면 된다(F-1 · [docs/12 §4](../../../docs/12-clipboard-formats.md)).
//!
//! ⚠️ 다만 **내용이 없는 곁다리(메타데이터)** 는 빼야 한다 — [`is_metadata_format`].
//! `CF_LOCALE`을 벤더로 세면 **Word에서 복사한 맨 텍스트가 "서식 있는 글"이 된다.**

use crate::item::{is_plain_format, ClipKind, Representation};

/// ★ **판정에 필요한 것은 이름과 크기뿐**이다.
///
/// 감시 계층이 읽어 온 [`RawRep`](crate::ports::RawRep)에는 `blob_id`가 없다 —
/// 그건 암호문 해시라 **키를 아는 저장 계층**만 만들 수 있다.
/// 이 트레이트가 없으면 감시 계층이 *"아직 모르는 값"* 을 억지로 채워야 한다.
pub trait RepInfo {
    /// OS가 준 포맷 이름.
    fn format(&self) -> &str;
    /// 바이트 크기(상한 판정용).
    fn bytes(&self) -> u64;
}

impl RepInfo for Representation {
    fn format(&self) -> &str {
        &self.format
    }
    fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl RepInfo for crate::ports::RawRep {
    fn format(&self) -> &str {
        &self.format
    }
    fn bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

// ─────────────────────────────────────────────── 포맷 분류(3-OS 한 곳)

/// 파일·폴더 목록 표현인가.
#[must_use]
pub fn is_files_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "CF_HDROP"
            | "FileGroupDescriptor"
            | "FileGroupDescriptorW"
            | "public.file-url"
            | "NSFilenamesPboardType"
            | "text/uri-list"
    )
}

/// HTML 표현인가. Windows는 `CF_HTML`이 아니라 **`HTML Format`** 이라는 이름으로 온다.
#[must_use]
pub fn is_html_format(fmt: &str) -> bool {
    matches!(fmt, "CF_HTML" | "HTML Format" | "public.html" | "text/html")
}

/// RTF 표현인가.
#[must_use]
pub fn is_rtf_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "Rich Text Format"
            | "Rich Text Format Without Objects"
            | "public.rtf"
            | "text/rtf"
            | "application/rtf"
    )
}

/// 래스터 이미지 표현인가(메타파일은 **아니다** — 우리가 못 그린다).
#[must_use]
pub fn is_bitmap_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "CF_DIB"
            | "CF_DIBV5"
            | "CF_BITMAP"
            | "PNG"
            | "public.png"
            | "public.tiff"
            | "image/png"
            | "image/bmp"
            | "image/tiff"
            | "image/jpeg"
            // ★ 08-27 실기 — PPT가 항상 함께 올리는 것들. 목록에 없어서 **벤더로 세어졌다**.
            //   `JFIF` = JPEG, `GIF`·SVG도 그냥 그림 인코딩이다.
            | "JFIF"
            | "JPEG"
            | "GIF"
            | "image/gif"
            | "image/svg+xml"
            | "image/webp"
            | "public.jpeg"
    )
}

/// ⚠️ **내용이 아니라 곁다리**인 표현 — 종류 판정에서 **없는 셈 친다**.
///
/// 이 목록이 비면 `CF_LOCALE` 하나 때문에 맨 텍스트가 서식 글로 분류된다.
/// 실기에서 새 이름을 만나면 여기 추가한다(→ **D-75**).
#[must_use]
pub fn is_metadata_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "CF_LOCALE"
            | "Preferred DropEffect"
            | "InShellDragLoop"
            | "DragContext"
            | "DragImageBits"
            | "UsingDefaultDragImage"
            | "IsShowingLayered"
            | "DragSourceHelperFlags"
            | "Object Descriptor"
            | "Link Source"
            | "Link Source Descriptor"
            | "ObjectLink"
            | "OwnerLink"
            | "Ole Private Data"
            // ★ 08-27 `watch` 실기에서 만난 것들 — 이게 없으면 **맨 텍스트가 리치로** 분류됐다.
            | "DataObject"
            | "Shell IDList Array"
            | "CanIncludeInClipboardHistory"
            | "CanUploadToCloudClipboard"
            | "ExcludeClipboardContentFromMonitorProcessing"
            // Chromium 계열(Chrome·Edge)이 **스스로 붙이는 내부 표식** — 내용이 아니다.
            // ⚠️ 브라우저 입력창에서 맨 텍스트를 복사하면 이것만 딸려 오므로,
            //    빼지 않으면 **평문이 "서식 있는 글"로** 분류된다.
            | "Chromium internal source URL"
            | "Chromium internal source RFH token"
            // 원격 데스크톱(rdpclip)·CopyQ가 붙이는 자기 표식 — 내용이 아니다(08-27 실기).
            | "Terminal Services Private Data"
            | "application/x-copyq-owner"
            | "msSourceUrl"
            | "com.apple.cocoa.pasteboard.source-app-id"
    )
}

/// 메타파일(벡터 그림) — 보관은 하지만 ★ **우리가 그리지 못한다**([docs/27 §2-3]).
#[must_use]
pub fn is_metafile_format(fmt: &str) -> bool {
    matches!(
        fmt,
        "CF_ENHMETAFILE" | "CF_METAFILEPICT" | "com.adobe.pdf" | "application/pdf"
    )
}

/// ★ **앱 고유 포맷인가** — 아는 표준도 곁다리도 아니면 벤더다([§2](#2--벤더-포맷을-목록으로-알아보지-않는다)).
#[must_use]
pub fn is_vendor_format(fmt: &str) -> bool {
    !fmt.is_empty()
        && !is_metadata_format(fmt)
        && !is_files_format(fmt)
        && !is_html_format(fmt)
        && !is_rtf_format(fmt)
        && !is_bitmap_format(fmt)
        && !is_metafile_format(fmt)
        && !is_plain_format(fmt)
}

// ─────────────────────────────────────────────── ① 종류 판정

/// 표현 이름들로 항목 종류를 정한다 — ★ **순서가 규칙 전체**다([§1](#1-종류-판정)).
///
/// `ClipKind::Color`는 여기서 나오지 않는다 — 색은 **평문 내용**을 봐야 알고(`#RRGGBB`),
/// 이 함수는 이름만 본다. 색 판별은 호출자가 평문을 얻은 뒤에 한다.
#[must_use]
pub fn classify<S: AsRef<str>>(formats: &[S]) -> ClipKind {
    let f = |pred: fn(&str) -> bool| formats.iter().any(|s| pred(s.as_ref()));

    // ① 파일이 가장 먼저 — 탐색기 복사에도 이름이 담긴 평문이 따라온다.
    if f(is_files_format) {
        return ClipKind::Files;
    }
    // ② 서식 있는 **글**이 실제로 있다.
    if f(is_html_format) || f(is_rtf_format) {
        return ClipKind::RichText;
    }
    // ③ 벤더(앱 고유) — ★ **글이 딸려 있는지**로 갈린다.
    if f(is_vendor_format) {
        if f(is_plain_format) {
            // Excel 범위 · 글 있는 PPT 도형 — 글이 있으니 "서식 있는 글"이 맞다.
            return ClipKind::RichText;
        }
        // ★ 08-27 실기 정정 2차 — 글이 하나도 없는 앱 개체는 **`Object`** 다.
        //   `Image`라고 하면 *"붙여넣으면 편집 가능한 도형"* 이라는 사실이 가려지고,
        //   `RichText`라고 하면 글이 없는데 거짓말이 된다(PPT `Art::GVML ClipFormat`).
        return ClipKind::Object;
    }
    // ④ 그림 — 벤더가 없는 순수 래스터(스크린샷 도구 등).
    if f(is_bitmap_format) || f(is_metafile_format) {
        return ClipKind::Image;
    }
    ClipKind::Text
}

/// ★ **내용을 담은 표현이 하나라도 있는가** — 없으면 **항목이 아니다**.
///
/// ⚠️ 08-27 실기에서 Excel이 지연 렌더링으로 클립보드를 다시 채우는 순간에 읽혀
/// **표현 0개짜리 항목**이 만들어졌다. rdpclip의 `Terminal Services Private Data`처럼
/// **곁다리만 있는** 경우도 마찬가지다.
///
/// 목록에 빈 줄이 쌓이면 사용자는 **제품이 고장 났다고 읽는다**.
#[must_use]
pub fn has_content<R: RepInfo>(reps: &[R]) -> bool {
    reps.iter().any(|r| !is_metadata_format(r.format()))
}

/// [`classify`]와 같되 **평문 내용까지** 본다 — 색 코드 한 줄이면 [`ClipKind::Color`].
///
/// 색으로 보는 조건은 좁게 잡는다: `#RGB` · `#RRGGBB` · `#RRGGBBAA` **한 덩어리만** 있을 때.
/// ⚠️ 넓히면 `#hashtag`가 든 문장이 전부 색이 된다.
#[must_use]
pub fn classify_with_text<S: AsRef<str>>(formats: &[S], plain: Option<&str>) -> ClipKind {
    let kind = classify(formats);
    if kind == ClipKind::Text {
        if let Some(t) = plain {
            if looks_like_color(t) {
                return ClipKind::Color;
            }
        }
    }
    kind
}

/// `#RGB`/`#RRGGBB`/`#RRGGBBAA` **하나만** 있는 문자열인가.
#[must_use]
pub fn looks_like_color(text: &str) -> bool {
    let t = text.trim();
    let Some(hex) = t.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

// ─────────────────────────────────────────────── ①-2 평문 디코드

/// ★ **평문 표현의 바이트를 글자로 푼다** — 포맷마다 인코딩이 다르다.
///
/// ⚠️ **`CF_UNICODETEXT`는 UTF-16LE다.** UTF-8로 읽으면
///
/// | 입력 | UTF-8로 읽으면 |
/// |---|---|
/// | `cargo run` (ASCII) | `c a r g o   r u n` — **NUL이 공백처럼 끼어 조용히 망가진다** |
/// | `평문 한 줄` (한글) | 디코드 실패 → **통째로 사라진다** |
///
/// 둘 다 오류를 내지 않는다는 게 고약한 점이다(08-27 `watch` 실기에서 발견).
///
/// `CF_TEXT`·`CF_OEMTEXT`는 시스템 코드 페이지라 우리가 정확히 못 푼다 —
/// Windows가 텍스트가 있으면 `CF_UNICODETEXT`를 **자동 합성**하므로 그쪽을 먼저 본다.
#[must_use]
pub fn decode_plain(format: &str, data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    let text = if format == "CF_UNICODETEXT" {
        // UTF-16LE — 홀수 바이트가 남으면 버린다(잘린 데이터).
        let units: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8(data.to_vec()).ok()?
    };
    // 클립보드 문자열은 널 종단이 붙어 온다 — 남기면 화면에 네모가 뜬다.
    let trimmed = text.trim_end_matches('\0');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// 평문 표현을 고르는 **우선순위** — 정확히 풀 수 있는 것이 먼저다.
///
/// ★ `CF_UNICODETEXT` > `public.utf8-plain-text`·`text/plain` > `CF_TEXT` > `CF_OEMTEXT`.
/// 코드 페이지에 묶인 `CF_TEXT`를 먼저 보면 한글이 깨진다.
#[must_use]
pub fn plain_rank(format: &str) -> Option<u8> {
    Some(match format {
        "CF_UNICODETEXT" => 0,
        "public.utf8-plain-text" | "text/plain" => 1,
        "CF_TEXT" => 2,
        "CF_OEMTEXT" => 3,
        f if is_plain_format(f) => 4,
        _ => return None,
    })
}

// ─────────────────────────────────────────────── ①-3 파일 경로 해석

/// `CF_HDROP`의 바이트를 경로 목록으로 푼다.
///
/// ## 구조
///
/// ```text
/// DROPFILES(20B)  ┌ 0  pFiles : u32  — 목록이 시작하는 오프셋
///                 │ 4  pt     : 8B   — 드롭 좌표(우리는 안 쓴다)
///                 │12  fNC    : u32
///                 └16  fWide  : u32  — ★ 0이 아니면 UTF-16LE
/// …목록: 널로 끊긴 문자열들 + 마지막에 널 하나 더(이중 널 종단)
/// ```
///
/// ⚠️ **`pFiles`를 20으로 가정하면 안 된다** — 구조체 뒤에 여백이 붙는 구현이 있다.
/// ⚠️ **`fWide`를 안 보면 ANSI 복사에서 글자가 깨진다**.
///
/// 잘린 데이터·이상한 오프셋에는 **빈 목록**을 준다(패닉 금지 — 클립보드는 남의 데이터다).
#[must_use]
pub fn parse_hdrop(data: &[u8]) -> Vec<String> {
    const HEADER: usize = 20;
    if data.len() < HEADER {
        return Vec::new();
    }
    let off = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let wide = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) != 0;
    if off < HEADER || off > data.len() {
        return Vec::new();
    }
    let body = &data[off..];
    let mut out = Vec::new();
    if wide {
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        for part in units.split(|u| *u == 0) {
            if part.is_empty() {
                continue;
            }
            out.push(String::from_utf16_lossy(part));
        }
    } else {
        for part in body.split(|b| *b == 0) {
            if part.is_empty() {
                continue;
            }
            // ANSI 코드 페이지는 정확히 못 푼다 — ASCII 범위만 신뢰한다.
            if let Ok(t) = std::str::from_utf8(part) {
                out.push(t.to_string());
            }
        }
    }
    out
}

/// `text/uri-list`(X11·Wayland) → 경로. `file://` 만 받는다.
#[must_use]
pub fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("file://"))
        .map(percent_decode)
        .collect()
}

/// `%20` 같은 퍼센트 인코딩을 되돌린다(URI 경로 전용 · 실패하면 원문 유지).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// 경로에서 **이름만** 뽑는다 — 목록에 전체 경로를 띄우면 길고 사생활이다.
#[must_use]
pub fn base_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

// ─────────────────────────────────────────────── ② 대표·썸네일 원본

/// 붙여넣기 사다리 꼭대기 — **벤더 > HTML > RTF > 비트맵 > 파일 > 평문**.
///
/// ★ 이 값은 *"원본 붙여넣기에서 이걸 쓴다"* 는 뜻이 **아니다**. 원본은 표현을
/// 전부 올린다([docs/26 §4-5](../../../docs/26-file-content-sharing.md) P-2).
/// 여기는 **용량 상한에서 무엇을 끝까지 지킬지**의 순위다.
#[must_use]
pub fn primary_index<R: RepInfo>(reps: &[R]) -> Option<usize> {
    let rank = |fmt: &str| -> u8 {
        if is_vendor_format(fmt) {
            0
        } else if is_html_format(fmt) {
            1
        } else if is_rtf_format(fmt) {
            2
        } else if is_bitmap_format(fmt) {
            3
        } else if is_files_format(fmt) {
            4
        } else if is_plain_format(fmt) {
            5
        } else {
            6
        }
    };
    reps.iter()
        .enumerate()
        .filter(|(_, r)| !is_metadata_format(r.format()))
        .min_by_key(|(i, r)| (rank(r.format()), *i))
        .map(|(i, _)| i)
}

/// 썸네일을 뽑을 표현 — ★ **PNG > `CF_DIBV5` > `CF_DIB` > 그 외 래스터**.
///
/// ⚠️ **`CF_DIB`는 알파를 잃는 일이 잦다** — 32bpp `BITMAPINFOHEADER`의 상위 바이트를
/// 앱마다 다르게 다룬다. PNG가 있으면 PNG가 정본인 이유다([docs/27 §4-1]).
///
/// ★ **PPT 도형 미리보기가 공짜인 지점이기도 하다** — GVML은 못 읽지만
/// Office가 같은 클립보드에 비트맵을 함께 넣어 준다.
#[must_use]
pub fn thumbnail_source<R: RepInfo>(reps: &[R]) -> Option<usize> {
    let rank = |fmt: &str| -> Option<u8> {
        match fmt {
            "PNG" | "public.png" | "image/png" => Some(0),
            "CF_DIBV5" => Some(1),
            "CF_DIB" | "CF_BITMAP" | "image/bmp" => Some(2),
            _ if is_bitmap_format(fmt) => Some(3), // TIFF·JPEG — 디코더가 없을 수 있다
            _ => None,
        }
    };
    reps.iter()
        .enumerate()
        .filter_map(|(i, r)| rank(r.format()).map(|k| (k, i)))
        .min()
        .map(|(_, i)| i)
}

// ─────────────────────────────────────────────── ③ 용량 상한

/// [`select_reps`] 결과.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RepSelection {
    /// 남길 표현 인덱스(원래 순서 유지).
    pub keep: Vec<usize>,
    /// 버릴 표현 인덱스(원래 순서 유지).
    pub dropped: Vec<usize>,
    /// 버린 바이트 합 — ★ **0이 아니면 항목에 표시한다**(C-3).
    pub dropped_bytes: u64,
    /// ⚠️ **필수 세트만으로도 상한을 넘었다** — 더 버릴 수 없어 그대로 둔 상태.
    pub over_budget: bool,
}

/// 용량 상한을 적용해 남길 표현을 고른다([docs/27 §7](../../../docs/27-capture-cases.md)).
///
/// | # | 규칙 |
/// |:--:|---|
/// | **C-1** | ★ **필수 세트는 항상 남긴다** — 대표 1 · HTML · RTF · 평문 · 썸네일용 1 · 파일 목록 |
/// | **C-2** | 상한을 넘으면 **필수가 아닌 것부터, 큰 것부터** 버린다 |
/// | **C-3** | 버린 양을 돌려준다 — ★ **조용히 버리면 한 달 뒤 붙여넣기가 이상해지고 이유를 알 수 없다** |
///
/// 곁다리(메타데이터)는 필수가 아니지만 **작아서 대개 살아남는다** — 굳이 먼저 버리지 않는다.
/// `limit_bytes == 0` = 상한 없음.
#[must_use]
pub fn select_reps<R: RepInfo>(reps: &[R], limit_bytes: u64) -> RepSelection {
    let total: u64 = reps.iter().map(RepInfo::bytes).sum();
    if limit_bytes == 0 || total <= limit_bytes {
        return RepSelection {
            keep: (0..reps.len()).collect(),
            dropped: Vec::new(),
            dropped_bytes: 0,
            over_budget: false,
        };
    }

    // C-1 — 필수 세트.
    let mut essential = vec![false; reps.len()];
    for i in primary_index(reps)
        .into_iter()
        .chain(thumbnail_source(reps))
    {
        essential[i] = true;
    }
    for (i, r) in reps.iter().enumerate() {
        if is_html_format(r.format())
            || is_rtf_format(r.format())
            || is_plain_format(r.format())
            || is_files_format(r.format())
        {
            essential[i] = true;
        }
    }

    // C-2 — 필수가 아닌 것을 **큰 것부터** 버린다(같으면 뒤쪽부터 — 결과가 결정적이도록).
    let mut candidates: Vec<usize> = (0..reps.len()).filter(|i| !essential[*i]).collect();
    candidates.sort_by_key(|i| (std::cmp::Reverse(reps[*i].bytes()), std::cmp::Reverse(*i)));

    let mut drop = vec![false; reps.len()];
    let mut cur = total;
    let mut dropped_bytes = 0;
    for i in candidates {
        if cur <= limit_bytes {
            break;
        }
        drop[i] = true;
        cur -= reps[i].bytes();
        dropped_bytes += reps[i].bytes();
    }

    RepSelection {
        keep: (0..reps.len()).filter(|i| !drop[*i]).collect(),
        dropped: (0..reps.len()).filter(|i| drop[*i]).collect(),
        dropped_bytes,
        // ⚠️ 필수만 남기고도 넘으면 그대로 둔다 — 필수를 버리면 붙여넣기가 망가진다.
        over_budget: cur > limit_bytes,
    }
}

// ─────────────────────────────────────────────── 미리보기

/// 미리보기를 만들지 못한 이유 — ★ **빈 상자만 두고 넘어가지 않는다**([DR-31]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewMissing {
    /// 그릴 수 있는 표현이 없다(메타파일만 있는 경우 등).
    NoRenderableRep,
    /// 디코더가 없는 이미지 포맷(v1은 PNG·BMP·DIB만) — 포맷 이름을 함께 남긴다.
    NoDecoder(&'static str),
    /// ★ 그릴 수 있는 래스터는 있는데 **축소본이 아직 안 만들어졌다**(T-14c 이전).
    ///
    /// `TooLarge`나 `NoDecoder`로 뭉뚱그리면 **원인을 잘못 짚게 된다** —
    /// 사용자는 파일이 너무 크거나 포맷을 못 읽는 줄 안다.
    ThumbMissing,
    /// 미리보기를 만들기엔 너무 크다.
    TooLarge,
}

impl PreviewMissing {
    /// 사용자에게 보일 짧은 사유(목록 배지).
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            PreviewMissing::NoRenderableRep => "미리보기 없음",
            PreviewMissing::NoDecoder(f) => f,
            PreviewMissing::ThumbMissing => "미리보기 준비 중",
            PreviewMissing::TooLarge => "미리보기 없음(용량)",
        }
    }
}

/// ★ **목록이 읽는 유일한 것**([docs/27 §6](../../../docs/27-capture-cases.md)).
///
/// 표현과 **따로** 두는 이유 세 가지:
///
/// 1. **속도** — 스크롤마다 3MB DIB를 디코드할 수 없다([docs/00 §2] 검색 ≤16ms)
/// 2. ★ **보안 경계가 한 지점에 모인다** — HTML 정제(스크립트·외부 참조 제거)를
///    **캡처 때 한 번**만 한다. 렌더 때마다 하면 언젠가 빠뜨린다
/// 3. **동기화가 싸진다** — 다른 기기엔 미리보기만 먼저 보낸다
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Preview {
    /// 잘라 둔 평문.
    Text(String),
    /// ★ **캡처 때 한 번 정제된** HTML 부분집합.
    Rich(String),
    /// 축소본 blob + 원본 크기.
    Thumb {
        /// 축소본 내용 주소.
        blob_id: [u8; 32],
        /// 원본 가로.
        w: u32,
        /// 원본 세로.
        h: u32,
    },
    /// 파일 이름 목록(경로 전체는 표현에 있다).
    Files(Vec<String>),
    /// 만들지 못했다 — ★ **왜인지를 함께 남긴다**.
    None(PreviewMissing),
}

impl Preview {
    /// 한 줄 보기에 쓸 평문 — 어떤 종류든 **무엇이든 준다**(빈 줄을 그리지 않는다).
    #[must_use]
    pub fn one_line(&self) -> String {
        match self {
            Preview::Text(t) | Preview::Rich(t) => t
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .into(),
            Preview::Thumb { w, h, .. } => format!("[이미지] {w}×{h}"),
            Preview::Files(names) => match names.split_first() {
                None => "[파일]".into(),
                Some((first, [])) => first.clone(),
                Some((first, rest)) => format!("{first} 외 {}개", rest.len()),
            },
            Preview::None(why) => why.reason().into(),
        }
    }
}

/// 평문을 목록용으로 자른다 — **문자 경계에서** 자르고 줄바꿈은 공백으로 접는다.
///
/// ⚠️ 바이트로 자르면 한글이 깨진다.
#[must_use]
pub fn clip_text(text: &str, max_chars: usize) -> String {
    let folded: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = folded.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(max_chars).collect();
    format!("{}…", head.trim_end())
}

// ─────────────────────────────────────────────── 파이프라인 조립

/// 만들어 둔 축소본 — `(내용 주소, 원본 가로, 원본 세로)`.
///
/// ★ 축소는 **디코더를 가진 층**이 한다 — 이 모듈은 순수해서 픽셀을 못 만진다.
pub type ThumbInfo = ([u8; 32], u32, u32);

/// 캡처 정책 — 호스트(설정)가 준다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CapturePolicy {
    /// 항목 하나가 차지할 수 있는 바이트 상한(0 = 무제한).
    pub max_item_bytes: u64,
    /// 목록 미리보기에 남길 글자 수.
    pub preview_chars: usize,
}

impl Default for CapturePolicy {
    /// 항목 32MB · 미리보기 200자 — [docs/14 §3-3](../../../docs/14-settings-registry.md) `cap.max_item_mb`.
    fn default() -> Self {
        Self {
            max_item_bytes: 32 * 1024 * 1024,
            preview_chars: 200,
        }
    }
}

/// 캡처가 만들어 낸 것 — 판정 결과 + **버려진 것에 대한 사실**.
///
/// ★ 표현을 **복제하지 않고 인덱스**로 돌려준다 — 원본은 호출자가 들고 있고,
/// 20MB짜리를 판정 한 번에 한 벌 더 만들 이유가 없다.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Captured {
    /// 종류.
    pub kind: ClipKind,
    /// 남길 표현의 **입력 인덱스**(원래 순서).
    pub keep: Vec<usize>,
    /// 목록이 읽는 것.
    pub preview: Preview,
    /// ★ 용량 때문에 버린 바이트 — **0이 아니면 항목에 표시한다**(C-3).
    pub dropped_bytes: u64,
    /// ⚠️ 필수 세트만으로도 상한을 넘었다.
    pub over_budget: bool,
}

/// ★ **클립보드 한 벌 → 항목 하나**([docs/27](../../../docs/27-capture-cases.md) 전체 규칙의 조립 지점).
///
/// `plain`은 평문 표현의 **내용**이다(있으면). 이 함수는 blob을 열지 못하므로
/// 호출자가 읽어서 넘긴다 — 그래야 여기가 순수 함수로 남는다.
/// `thumb`은 썸네일을 실제로 만들 수 있었을 때 그 결과다(디코더가 없으면 `None`).
///
/// ⚠️ **민감 표식(`concealed`)은 여기서 보지 않는다** — 저장 여부는 이 함수보다 **앞**에서
/// 결정된다([`crate::ports::ClipSnapshot`] · FR-S-1 fail-closed). 여기까지 왔다면 이미 통과한 것이다.
#[must_use]
pub fn capture<R: RepInfo>(
    reps: &[R],
    plain: Option<&str>,
    thumb: Option<ThumbInfo>,
    rich_html: Option<&str>,
    file_names: &[String],
    policy: CapturePolicy,
) -> Captured {
    let formats: Vec<&str> = reps.iter().map(RepInfo::format).collect();
    let kind = classify_with_text(&formats, plain);

    let sel = select_reps(reps, policy.max_item_bytes);
    // 남긴 것들의 **이름만** 미리보기 판정에 넘긴다(썸네일 원본 고르기).
    let kept_names: Vec<KeptName> = sel
        .keep
        .iter()
        .map(|i| KeptName(reps[*i].format().to_string()))
        .collect();

    let preview = build_preview(
        kind,
        plain,
        thumb,
        rich_html,
        file_names,
        &kept_names,
        policy,
    );

    Captured {
        kind,
        keep: sel.keep,
        preview,
        dropped_bytes: sel.dropped_bytes,
        over_budget: sel.over_budget,
    }
}

/// 미리보기 판정에만 쓰는 가벼운 이름 상자(크기는 필요 없다).
struct KeptName(String);

impl RepInfo for KeptName {
    fn format(&self) -> &str {
        &self.0
    }
    fn bytes(&self) -> u64 {
        0
    }
}

/// 종류에 맞는 미리보기를 고른다 — ★ **못 만들면 이유를 남긴다**.
fn build_preview<R: RepInfo>(
    kind: ClipKind,
    plain: Option<&str>,
    thumb: Option<ThumbInfo>,
    rich_html: Option<&str>,
    file_names: &[String],
    kept: &[R],
    policy: CapturePolicy,
) -> Preview {
    match kind {
        ClipKind::Files => {
            if file_names.is_empty() {
                Preview::None(PreviewMissing::NoRenderableRep)
            } else {
                Preview::Files(file_names.to_vec())
            }
        }
        // 개체는 그림처럼 보여 준다 — 함께 온 비트맵이 곧 미리보기다.
        ClipKind::Image | ClipKind::Object => image_preview(thumb, kept),
        // 리치는 정제된 HTML이 있으면 그걸, 없으면 평문으로 **강등**한다.
        // ★ 도형처럼 글자가 없는 리치는 썸네일이 미리보기다.
        ClipKind::RichText => match (rich_html, plain) {
            (Some(h), _) if !h.trim().is_empty() => {
                Preview::Rich(clip_text(h, policy.preview_chars))
            }
            (_, Some(t)) if !t.trim().is_empty() => {
                Preview::Text(clip_text(t, policy.preview_chars))
            }
            _ => image_preview(thumb, kept),
        },
        ClipKind::Text | ClipKind::Color => match plain {
            Some(t) if !t.trim().is_empty() => Preview::Text(clip_text(t, policy.preview_chars)),
            _ => Preview::None(PreviewMissing::NoRenderableRep),
        },
    }
}

/// 썸네일이 있으면 그것, 없으면 **왜 없는지**.
fn image_preview<R: RepInfo>(thumb: Option<ThumbInfo>, kept: &[R]) -> Preview {
    if let Some((blob_id, w, h)) = thumb {
        return Preview::Thumb { blob_id, w, h };
    }
    // 래스터는 있는데 썸네일이 없다 = 디코더가 없다. 어느 포맷인지 밝힌다.
    match thumbnail_source(kept).map(|i| kept[i].format()) {
        Some("image/jpeg") => Preview::None(PreviewMissing::NoDecoder("JPEG")),
        Some("public.tiff" | "image/tiff") => Preview::None(PreviewMissing::NoDecoder("TIFF")),
        Some(_) => Preview::None(PreviewMissing::ThumbMissing),
        None => Preview::None(PreviewMissing::NoRenderableRep),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(fmt: &str, bytes: u64) -> Representation {
        Representation {
            format: fmt.into(),
            blob_id: [0; 32],
            bytes,
        }
    }

    /// ★ **PPT 도형 2개** — 이 프로젝트에서 가장 중요한 판정.
    ///
    /// 비트맵을 벤더보다 먼저 보면 도형이 "이미지"가 된다([docs/27 §1]).
    #[test]
    fn ppt_shapes_are_rich_not_image() {
        let formats = [
            "Art::GVML ClipFormat",
            "PowerPoint 14.0 Slides Package",
            "Object Descriptor",
            "CF_ENHMETAFILE",
            "CF_DIB", // ← 함정: 도형 복사에도 비트맵이 들어 있다
            "CF_DIBV5",
            "HTML Format",
            "Rich Text Format",
            "CF_UNICODETEXT",
        ];
        assert_eq!(
            classify(&formats),
            ClipKind::RichText,
            "★ 도형이 이미지로 분류되면 검색·필터·보관 정책이 전부 틀어진다"
        );
    }

    /// 같은 항목의 **썸네일은 비트맵에서** 온다 — GVML을 읽지 않아도 미리보기가 나온다.
    #[test]
    fn ppt_thumbnail_comes_from_the_bitmap_office_left_for_us() {
        let reps = [
            rep("Art::GVML ClipFormat", 40_000),
            rep("CF_ENHMETAFILE", 90_000),
            rep("CF_DIB", 120_000),
            rep("CF_UNICODETEXT", 60),
        ];
        let i = thumbnail_source(&reps).expect("비트맵이 있으면 썸네일 원본이 있다");
        assert_eq!(reps[i].format, "CF_DIB");
        // 대표는 여전히 벤더다.
        let p = primary_index(&reps).unwrap();
        assert_eq!(reps[p].format, "Art::GVML ClipFormat");
    }

    /// PNG가 있으면 PNG가 정본 — `CF_DIB`는 알파를 잃는다.
    #[test]
    fn png_beats_dib_for_thumbnail() {
        let reps = [rep("CF_DIB", 100), rep("PNG", 100), rep("CF_DIBV5", 100)];
        let i = thumbnail_source(&reps).unwrap();
        assert_eq!(reps[i].format, "PNG");
    }

    /// ★ 탐색기 복사 — 파일 이름이 담긴 평문이 따라와도 **Files**여야 한다.
    #[test]
    fn explorer_copy_is_files_even_with_text() {
        let formats = [
            "CF_HDROP",
            "Preferred DropEffect",
            "Shell IDList Array",
            "CF_UNICODETEXT",
        ];
        assert_eq!(classify(&formats), ClipKind::Files);
    }

    /// ★ `CF_UNICODETEXT`는 **UTF-16LE**다 — UTF-8로 읽으면 조용히 망가진다.
    ///
    /// 08-27 `watch` 실기에서 `cargo run`이 `c a r g o   r u n`으로 찍혀 발각됐다.
    #[test]
    fn unicode_text_is_utf16() {
        // "가나" + 널 종단
        let mut bytes = Vec::new();
        for u in "가나".encode_utf16().chain(std::iter::once(0)) {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(
            decode_plain("CF_UNICODETEXT", &bytes).as_deref(),
            Some("가나")
        );

        // ASCII도 마찬가지 — UTF-8로 읽으면 NUL이 사이사이 낀다.
        let mut ascii = Vec::new();
        for u in "ab".encode_utf16() {
            ascii.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(
            decode_plain("CF_UNICODETEXT", &ascii).as_deref(),
            Some("ab")
        );
        assert_ne!(
            String::from_utf8_lossy(&ascii),
            "ab",
            "★ UTF-8로 읽으면 다른 것이 나온다는 사실 자체를 박아 둔다"
        );
    }

    /// 다른 포맷은 UTF-8이고, 널 종단은 잘라 낸다.
    #[test]
    fn other_plain_formats_are_utf8() {
        assert_eq!(
            decode_plain("public.utf8-plain-text", b"hi\0").as_deref(),
            Some("hi")
        );
        assert!(decode_plain("CF_TEXT", &[]).is_none(), "빈 데이터는 None");
        assert!(
            decode_plain("CF_TEXT", &[0xFF, 0xFE]).is_none(),
            "UTF-8이 아니면 None — 억지로 읽지 않는다"
        );
    }

    /// ★ 정확히 풀 수 있는 평문이 먼저다 — `CF_TEXT`를 먼저 보면 한글이 깨진다.
    #[test]
    fn unicode_text_outranks_codepage_text() {
        let u = plain_rank("CF_UNICODETEXT").unwrap();
        assert!(u < plain_rank("CF_TEXT").unwrap());
        assert!(u < plain_rank("CF_OEMTEXT").unwrap());
        assert!(plain_rank("CF_DIB").is_none());
    }

    /// ★ 08-27 실기 회귀 — PowerShell `Set-Clipboard` 한 줄이 **리치로 분류됐다**.
    ///
    /// 원인은 `DataObject`(8바이트 OLE 표식)가 "모르는 포맷 = 벤더"에 걸린 것.
    #[test]
    fn powershell_plain_copy_is_text_not_rich() {
        let formats = [
            "DataObject",
            "CF_UNICODETEXT",
            "Ole Private Data",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
        ];
        assert_eq!(
            classify(&formats),
            ClipKind::Text,
            "★ 곁다리 하나 때문에 맨 텍스트가 서식 글이 되면 안 된다"
        );
    }

    /// ★ 08-27 실기 — Edge 복사에서 나온 Chromium 내부 표식.
    ///
    /// 이 항목 자체는 `HTML Format`이 있어 리치가 맞았지만, **브라우저 입력창에서
    /// 맨 텍스트를 복사하면** 이 표식만 딸려 온다 → 평문이 리치로 분류된다.
    #[test]
    fn chromium_internal_markers_are_metadata() {
        for f in [
            "Chromium internal source URL",
            "Chromium internal source RFH token",
        ] {
            assert!(is_metadata_format(f), "{f}는 곁다리여야 한다");
        }
        // 브라우저 입력창 복사 = 평문 + 내부 표식뿐.
        assert_eq!(
            classify(&[
                "Chromium internal source URL",
                "Chromium internal source RFH token",
                "CF_UNICODETEXT",
                "CF_LOCALE",
                "CF_TEXT",
                "CF_OEMTEXT",
            ]),
            ClipKind::Text
        );
        // 웹 본문 복사는 여전히 리치(HTML이 있다).
        assert_eq!(
            classify(&[
                "HTML Format",
                "Chromium internal source URL",
                "CF_UNICODETEXT"
            ]),
            ClipKind::RichText
        );
    }

    /// ⚠️ 이름에 "Source"가 들어간다고 곁다리로 몰면 **진짜 내용을 버린다**.
    ///
    /// `Embed Source`는 OLE **개체 본문**이고 `Link Source`는 연결 정보다.
    /// 그래서 규칙이 아니라 **명시 목록**으로만 거른다.
    #[test]
    fn source_in_name_is_not_a_rule() {
        assert!(
            !is_metadata_format("Embed Source"),
            "★ 개체 본문을 버리면 안 된다"
        );
        assert!(is_vendor_format("Embed Source"));
    }

    /// 클립보드 기록 표식도 곁다리다 — 있다고 리치가 되면 안 된다.
    #[test]
    fn clipboard_history_markers_are_metadata() {
        for f in [
            "CanIncludeInClipboardHistory",
            "CanUploadToCloudClipboard",
            "Shell IDList Array",
            "DataObject",
        ] {
            assert!(is_metadata_format(f), "{f}는 곁다리여야 한다");
            assert!(!is_vendor_format(f), "{f}가 벤더로 세어졌다");
        }
    }

    /// ★ 곁다리를 벤더로 세면 **맨 텍스트가 서식 글이 된다**.
    #[test]
    fn metadata_only_text_stays_text() {
        let formats = ["CF_UNICODETEXT", "CF_TEXT", "CF_LOCALE", "Ole Private Data"];
        assert_eq!(
            classify(&formats),
            ClipKind::Text,
            "★ CF_LOCALE 하나 때문에 텍스트가 리치가 되면 안 된다"
        );
    }

    /// 색·크기가 적용된 텍스트 — HTML/RTF가 있으면 리치.
    #[test]
    fn styled_text_is_rich() {
        assert_eq!(
            classify(&["HTML Format", "Rich Text Format", "CF_UNICODETEXT"]),
            ClipKind::RichText
        );
        assert_eq!(
            classify(&["public.rtf", "public.utf8-plain-text"]),
            ClipKind::RichText
        );
    }

    /// 순수 이미지 — 평문이 없다.
    #[test]
    fn plain_image_is_image() {
        assert_eq!(classify(&["PNG", "CF_DIB", "CF_DIBV5"]), ClipKind::Image);
        assert_eq!(classify(&["public.png", "public.tiff"]), ClipKind::Image);
    }

    /// 우리가 못 그리는 메타파일만 있어도 **이미지로는 분류**한다(보관은 한다).
    #[test]
    fn metafile_only_is_image_kind() {
        assert_eq!(classify(&["CF_ENHMETAFILE"]), ClipKind::Image);
        assert!(thumbnail_source(&[rep("CF_ENHMETAFILE", 10)]).is_none());
    }

    #[test]
    fn empty_clipboard_is_text() {
        let none: [&str; 0] = [];
        assert_eq!(classify(&none), ClipKind::Text);
    }

    /// 색 판별은 **좁게** — 문장 속 `#`은 색이 아니다.
    #[test]
    fn color_detection_is_narrow() {
        let f = ["CF_UNICODETEXT"];
        assert_eq!(classify_with_text(&f, Some("#1E90FF")), ClipKind::Color);
        assert_eq!(classify_with_text(&f, Some("  #abc  ")), ClipKind::Color);
        assert_eq!(classify_with_text(&f, Some("#11223344")), ClipKind::Color);
        assert_eq!(
            classify_with_text(&f, Some("#태그 붙은 문장")),
            ClipKind::Text
        );
        assert_eq!(
            classify_with_text(&f, Some("#1E90FF 로 칠하자")),
            ClipKind::Text
        );
        assert_eq!(classify_with_text(&f, Some("#12345")), ClipKind::Text);
        // 리치는 색으로 강등되지 않는다.
        assert_eq!(
            classify_with_text(&["HTML Format"], Some("#1E90FF")),
            ClipKind::RichText
        );
    }

    /// 상한 아래면 아무것도 안 버린다.
    #[test]
    fn under_limit_keeps_everything() {
        let reps = [rep("PNG", 100), rep("CF_UNICODETEXT", 10)];
        let s = select_reps(&reps, 1_000);
        assert_eq!(s.keep, vec![0, 1]);
        assert!(s.dropped.is_empty() && s.dropped_bytes == 0 && !s.over_budget);
    }

    /// 상한 0 = 무제한.
    #[test]
    fn zero_limit_means_unlimited() {
        let reps = [rep("PNG", 9_999_999)];
        assert!(select_reps(&reps, 0).dropped.is_empty());
    }

    /// ★ 필수 세트는 살아남고, 무거운 곁가지부터 버린다.
    #[test]
    fn drops_heavy_non_essentials_first() {
        let reps = [
            rep("Art::GVML ClipFormat", 1_000),           // 대표 — 필수
            rep("PowerPoint 14.0 Slides Package", 8_000), // 벤더지만 대표가 아니다
            rep("CF_ENHMETAFILE", 5_000),                 // 그릴 수 없다 — 먼저 버릴 후보
            rep("CF_DIB", 2_000),                         // 썸네일 원본 — 필수
            rep("HTML Format", 300),                      // 필수
            rep("CF_UNICODETEXT", 50),                    // 필수
        ];
        let s = select_reps(&reps, 4_000);
        let kept: Vec<&str> = s.keep.iter().map(|i| reps[*i].format.as_str()).collect();
        assert!(kept.contains(&"Art::GVML ClipFormat"), "대표가 사라졌다");
        assert!(kept.contains(&"CF_DIB"), "썸네일 원본이 사라졌다");
        assert!(kept.contains(&"HTML Format") && kept.contains(&"CF_UNICODETEXT"));
        assert!(
            !kept.contains(&"PowerPoint 14.0 Slides Package"),
            "가장 큰 곁가지가 남았다"
        );
        assert_eq!(s.dropped_bytes, 8_000 + 5_000);
        assert!(!s.over_budget);
    }

    /// ★ 필수만으로 상한을 넘으면 **그대로 둔다** — 필수를 버리면 붙여넣기가 망가진다.
    #[test]
    fn essentials_are_never_dropped_even_over_budget() {
        let reps = [
            rep("Art::GVML ClipFormat", 50_000),
            rep("CF_UNICODETEXT", 50),
        ];
        let s = select_reps(&reps, 1_000);
        assert_eq!(s.keep, vec![0, 1], "필수는 버리지 않는다");
        assert_eq!(s.dropped_bytes, 0);
        assert!(s.over_budget, "★ 넘었다는 사실을 알려야 한다");
    }

    /// 같은 크기면 결과가 **결정적**이다(같은 입력 → 같은 출력).
    #[test]
    fn selection_is_deterministic() {
        let reps = [
            rep("CF_UNICODETEXT", 10),
            rep("Vendor A", 100),
            rep("Vendor B", 100),
            rep("Vendor C", 100),
        ];
        let a = select_reps(&reps, 150);
        let b = select_reps(&reps, 150);
        assert_eq!(a, b);
    }

    /// 한 줄 보기는 **어떤 종류든 무엇이든** 준다 — 빈 줄을 그리지 않는다.
    #[test]
    fn one_line_never_empty_for_known_kinds() {
        assert_eq!(Preview::Text("  \n첫 줄\n둘째".into()).one_line(), "첫 줄");
        assert_eq!(
            Preview::Thumb {
                blob_id: [0; 32],
                w: 1920,
                h: 1080
            }
            .one_line(),
            "[이미지] 1920×1080"
        );
        assert_eq!(Preview::Files(vec!["a.txt".into()]).one_line(), "a.txt");
        assert_eq!(
            Preview::Files(vec!["a.txt".into(), "b".into(), "c".into()]).one_line(),
            "a.txt 외 2개"
        );
        assert!(!Preview::None(PreviewMissing::NoRenderableRep)
            .one_line()
            .is_empty());
    }

    /// 디코더가 없으면 **포맷 이름을 남긴다** — "미리보기 없음"만으로는 이유를 모른다.
    #[test]
    fn missing_preview_says_why() {
        assert_eq!(PreviewMissing::NoDecoder("JPEG").reason(), "JPEG");
        assert!(!PreviewMissing::TooLarge.reason().is_empty());
    }

    /// ⚠️ 자를 때 **문자 경계**를 지킨다 — 바이트로 자르면 한글이 깨진다.
    #[test]
    fn clip_text_respects_char_boundaries() {
        let out = clip_text("가나다라마바사", 3);
        assert_eq!(out, "가나다…");
        assert!(out.chars().count() == 4);
    }

    /// 줄바꿈·제어문자는 공백으로 접는다(목록 한 줄이 깨지지 않게).
    #[test]
    fn clip_text_folds_control_chars() {
        assert_eq!(clip_text("a\nb\tc", 10), "a b c");
        assert_eq!(clip_text("   \n  ", 10), "");
    }

    #[test]
    fn clip_text_keeps_short_text_as_is() {
        assert_eq!(clip_text("짧다", 10), "짧다");
    }

    /// 대표는 곁다리를 고르지 않는다.
    #[test]
    fn primary_skips_metadata() {
        let reps = [rep("CF_LOCALE", 4), rep("CF_UNICODETEXT", 10)];
        let i = primary_index(&reps).unwrap();
        assert_eq!(reps[i].format, "CF_UNICODETEXT");
    }

    #[test]
    fn primary_of_empty_is_none() {
        let none: [Representation; 0] = [];
        assert!(primary_index(&none).is_none());
        assert!(thumbnail_source(&none).is_none());
    }

    // ───────────────────────── docs/27 네 케이스 전수 ─────────────────────────

    const P: CapturePolicy = CapturePolicy {
        max_item_bytes: 0,
        preview_chars: 200,
    };

    /// 케이스 ① — PPT 글상자 2개. ★ 리치로 분류되고 **썸네일이 미리보기**다.
    #[test]
    fn case1_ppt_shapes() {
        let reps = [
            rep("Art::GVML ClipFormat", 40_000),
            rep("CF_DIB", 120_000),
            rep("HTML Format", 800),
            rep("CF_UNICODETEXT", 60),
        ];
        let c = capture(
            &reps,
            Some("『세방전지㈜ ERP 시스템 고도화 구축』\n요구사항 분석 (MP)"),
            Some(([7; 32], 640, 180)),
            None,
            &[],
            P,
        );
        assert_eq!(c.kind, ClipKind::RichText);
        // 글자가 있으므로 평문 미리보기가 먼저다(썸네일은 일반 보기가 따로 쓴다).
        assert!(matches!(c.preview, Preview::Text(_)));
        assert!(c.one_line_contains("세방전지"));
    }

    /// ★ **PPT 도형(글 없음)은 `Object`** 다 — 08-27 실기의 [1]·[4]·[6].
    ///
    /// `Image`라고 하면 *"붙여넣으면 편집 가능한 도형"* 이라는 사실이 가려지고,
    /// `RichText`라고 하면 글이 하나도 없는데 거짓말이다.
    #[test]
    fn ppt_shape_without_text_is_object() {
        let reps = [
            rep("DataObject", 8),
            rep("Object Descriptor", 352),
            rep("PowerPoint 12.0 Internal Shapes", 8),
            rep("Art::GVML ClipFormat", 4_300),
            rep("image/svg+xml", 1_800),
            rep("PNG", 6_700),
            rep("JFIF", 19_600),
            rep("GIF", 3_600),
            rep("CF_DIB", 448_000),
            rep("CF_DIBV5", 448_000),
        ];
        let c = capture(&reps, None, Some(([3; 32], 320, 240)), None, &[], P);
        assert_eq!(c.kind, ClipKind::Object);
        // 미리보기는 그림처럼 — 함께 온 비트맵이 곧 미리보기다.
        assert_eq!(
            c.preview,
            Preview::Thumb {
                blob_id: [3; 32],
                w: 320,
                h: 240
            }
        );
    }

    /// ★ 반대로 **스크린샷 도구**는 진짜 `Image`다 — 08-27 실기의 [2](Greenshot).
    ///
    /// 진짜 벤더 포맷이 없고 그림 인코딩만 있다.
    #[test]
    fn screenshot_tool_copy_is_image() {
        let reps = [
            rep("DataObject", 8),
            rep("PNG", 23_500),
            rep("CF_DIB", 1_600_000),
            rep("Ole Private Data", 216),
            rep("CF_DIBV5", 1_600_000),
        ];
        assert_eq!(
            capture(&reps, None, None, None, &[], P).kind,
            ClipKind::Image
        );
    }

    /// ★ `JFIF`·`GIF`·SVG는 **그림 인코딩**이지 벤더가 아니다(08-27 실기).
    ///
    /// 목록에 없으면 PPT 복사에서 벤더 수를 부풀려 판정이 흔들린다.
    #[test]
    fn common_image_encodings_are_not_vendor() {
        for f in [
            "JFIF",
            "JPEG",
            "GIF",
            "image/gif",
            "image/svg+xml",
            "image/webp",
        ] {
            assert!(is_bitmap_format(f), "{f}는 그림 포맷이다");
            assert!(!is_vendor_format(f), "{f}가 벤더로 세어졌다");
        }
    }

    /// PPT **표**는 HTML이 있으므로 여전히 `RichText`다 — 실기의 [3].
    #[test]
    fn ppt_table_with_html_is_rich() {
        let reps = [
            rep("Art::Table ClipFormat", 7_100),
            rep("HTML Format", 48_800),
            rep("PNG", 34_000),
            rep("CF_DIB", 2_100_000),
        ];
        assert_eq!(
            capture(&reps, None, None, Some("<table>…</table>"), &[], P).kind,
            ClipKind::RichText
        );
    }

    /// Excel 범위는 글이 있으므로 `RichText` — 실기의 [13]·[15].
    #[test]
    fn excel_range_is_rich() {
        let reps = [
            rep("Biff12", 7_000),
            rep("XML Spreadsheet", 4_200),
            rep("HTML Format", 6_100),
            rep("CF_UNICODETEXT", 1_010),
            rep("Rich Text Format", 9_500),
            rep("CF_DIB", 1_100_000),
        ];
        assert_eq!(
            capture(&reps, Some("수요계획"), None, None, &[], P).kind,
            ClipKind::RichText
        );
    }

    /// ★ **내용 표현이 없으면 항목이 아니다** — 실기의 [14](Excel 0개)·[9](rdpclip).
    #[test]
    fn metadata_only_snapshot_has_no_content() {
        let none: [Representation; 0] = [];
        assert!(!has_content(&none), "빈 클립보드는 항목이 아니다");

        let only_meta = [
            rep("DataObject", 8),
            rep("Terminal Services Private Data", 4),
            rep("Ole Private Data", 168),
        ];
        assert!(
            !has_content(&only_meta),
            "★ 곁다리만 있는 것도 항목이 아니다 — 목록에 빈 줄이 쌓이면 고장으로 읽힌다"
        );

        let real = [rep("DataObject", 8), rep("CF_UNICODETEXT", 20)];
        assert!(has_content(&real));
    }

    /// 반면 **글이 딸린** 벤더는 여전히 리치다 — PPT 도형이 그림으로 떨어지면 안 된다.
    #[test]
    fn vendor_with_text_stays_rich() {
        let reps = [
            rep("Art::GVML ClipFormat", 9_000),
            rep("CF_DIB", 50_000),
            rep("CF_UNICODETEXT", 40),
        ];
        let c = capture(&reps, Some("도형 속 글"), None, None, &[], P);
        assert_eq!(c.kind, ClipKind::RichText);
    }

    /// 글도 그림도 없는 **순수 개체**도 `Object`다(되돌려 주는 것 말고 할 게 없다).
    #[test]
    fn bare_object_is_object() {
        assert_eq!(classify(&["Embed Source"]), ClipKind::Object);
    }

    /// ★ 래스터는 있는데 썸네일이 없으면 **"준비 중"** 이라고 말한다.
    ///
    /// `용량 초과`나 `디코더 없음`으로 뭉뚱그리면 사용자가 원인을 잘못 짚는다.
    #[test]
    fn missing_thumbnail_is_not_reported_as_too_large() {
        let reps = [rep("CF_DIB", 300_000)];
        let c = capture(&reps, None, None, None, &[], P);
        assert_eq!(c.preview, Preview::None(PreviewMissing::ThumbMissing));
        assert_ne!(c.preview, Preview::None(PreviewMissing::TooLarge));
    }

    /// 케이스 ② — 색·크기가 적용된 텍스트. 정제된 HTML이 있으면 그것이 미리보기다.
    #[test]
    fn case2_styled_text() {
        let reps = [
            rep("HTML Format", 2_400),
            rep("Rich Text Format", 3_100),
            rep("CF_UNICODETEXT", 220),
        ];
        let c = capture(
            &reps,
            Some("* 공장별 SKU에 대해서 MP계획 수립"),
            None,
            Some("<b>* 공장별 SKU</b>에 대해서 MP계획 수립"),
            &[],
            P,
        );
        assert_eq!(c.kind, ClipKind::RichText);
        assert!(matches!(c.preview, Preview::Rich(_)), "{:?}", c.preview);
    }

    /// ★ 정제된 HTML이 없으면 **평문으로 강등**한다 — 조용히 틀리게 그리지 않는다.
    #[test]
    fn case2_falls_back_to_plain_when_sanitize_failed() {
        let reps = [rep("HTML Format", 100), rep("CF_UNICODETEXT", 20)];
        let c = capture(&reps, Some("본문"), None, None, &[], P);
        assert_eq!(c.preview, Preview::Text("본문".into()));
    }

    /// 케이스 ③ — 이미지.
    #[test]
    fn case3_image() {
        let reps = [rep("PNG", 412_000), rep("CF_DIB", 8_100_000)];
        let c = capture(&reps, None, Some(([9; 32], 1920, 1080)), None, &[], P);
        assert_eq!(c.kind, ClipKind::Image);
        assert_eq!(c.preview.one_line(), "[이미지] 1920×1080");
    }

    /// ★ 디코더가 없으면 **어느 포맷인지 밝힌다** — 보관은 그대로 한다.
    #[test]
    fn case3_undecodable_image_says_which_format() {
        let reps = [rep("image/jpeg", 300_000)];
        let c = capture(&reps, None, None, None, &[], P);
        assert_eq!(c.kind, ClipKind::Image);
        assert_eq!(c.preview, Preview::None(PreviewMissing::NoDecoder("JPEG")));
        assert_eq!(c.keep.len(), 1, "★ 못 그린다고 안 담지 않는다");
    }

    /// ★ `CF_HDROP` — 헤더 오프셋과 `fWide`를 **둘 다** 봐야 한다.
    #[test]
    fn hdrop_wide_parses_paths() {
        let mut d = vec![0u8; 20];
        d[0] = 20; // pFiles = 20
        d[16] = 1; // fWide = TRUE
        for p in ["C:\\a\\보고서.xlsx", "C:\\b.txt"] {
            for u in p.encode_utf16() {
                d.extend_from_slice(&u.to_le_bytes());
            }
            d.extend_from_slice(&0u16.to_le_bytes());
        }
        d.extend_from_slice(&0u16.to_le_bytes()); // 이중 널 종단
        let got = parse_hdrop(&d);
        assert_eq!(got, vec!["C:\\a\\보고서.xlsx", "C:\\b.txt"]);
        assert_eq!(base_name(&got[0]), "보고서.xlsx");
    }

    /// ⚠️ 오프셋이 20이 아닐 수 있다 — 가정하면 경로가 잘린다.
    #[test]
    fn hdrop_honours_offset() {
        let mut d = vec![0u8; 24]; // 헤더 뒤 4바이트 여백
        d[0] = 24;
        d[16] = 1;
        for u in "C:\\x.txt".encode_utf16() {
            d.extend_from_slice(&u.to_le_bytes());
        }
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(parse_hdrop(&d), vec!["C:\\x.txt"]);
    }

    /// ANSI(`fWide` = 0)도 읽는다.
    #[test]
    fn hdrop_ansi_parses() {
        let mut d = vec![0u8; 20];
        d[0] = 20;
        d.extend_from_slice(b"C:\\a.txt\0C:\\b.txt\0\0");
        assert_eq!(parse_hdrop(&d), vec!["C:\\a.txt", "C:\\b.txt"]);
    }

    /// ★ 잘린 데이터에 **패닉하지 않는다** — 클립보드는 남의 데이터다.
    #[test]
    fn hdrop_rejects_garbage_without_panic() {
        assert!(parse_hdrop(&[]).is_empty());
        assert!(parse_hdrop(&[1, 2, 3]).is_empty());
        let mut bad = vec![0u8; 20];
        bad[0] = 200; // 데이터 밖을 가리키는 오프셋
        assert!(parse_hdrop(&bad).is_empty());
        let mut zero = vec![0u8; 20]; // 오프셋 0 = 헤더 안쪽
        zero[16] = 1;
        assert!(parse_hdrop(&zero).is_empty());
    }

    /// `text/uri-list` — `file://` 만 받고 퍼센트 인코딩을 되돌린다.
    #[test]
    fn uri_list_parses_file_urls() {
        let text = "# comment\nfile:///home/u/a%20b.txt\nhttps://example.com/x\nfile:///tmp/c\n";
        assert_eq!(
            parse_uri_list(text),
            vec!["/home/u/a b.txt".to_string(), "/tmp/c".to_string()]
        );
    }

    #[test]
    fn base_name_handles_both_separators() {
        assert_eq!(base_name("C:\\a\\b.txt"), "b.txt");
        assert_eq!(base_name("/home/u/c.md"), "c.md");
        assert_eq!(base_name("d.txt"), "d.txt");
        assert_eq!(base_name("C:\\folder\\"), "folder");
    }

    /// 케이스 ④ — 파일.
    #[test]
    fn case4_files() {
        let reps = [
            rep("CF_HDROP", 300),
            rep("Preferred DropEffect", 4),
            rep("CF_UNICODETEXT", 120),
        ];
        let names = vec!["보고서.xlsx".to_string(), "a.png".into(), "b.txt".into()];
        let c = capture(&reps, Some("D:\\문서\\보고서.xlsx"), None, None, &names, P);
        assert_eq!(c.kind, ClipKind::Files);
        assert_eq!(c.preview.one_line(), "보고서.xlsx 외 2개");
    }

    /// ★ 용량 상한이 걸리면 **버린 양을 알려 준다** — 조용히 버리지 않는다.
    #[test]
    fn capture_reports_what_it_dropped() {
        let reps = [
            rep("Art::GVML ClipFormat", 1_000),
            rep("PowerPoint 14.0 Slides Package", 9_000_000),
            rep("CF_DIB", 2_000),
            rep("CF_UNICODETEXT", 40),
        ];
        let c = capture(
            &reps,
            Some("도형"),
            Some(([1; 32], 10, 10)),
            None,
            &[],
            CapturePolicy {
                max_item_bytes: 100_000,
                preview_chars: 200,
            },
        );
        assert_eq!(c.dropped_bytes, 9_000_000);
        assert!(!c.over_budget);
        assert_eq!(c.keep.len(), 3, "필수 세트는 남는다");
    }

    /// 빈 클립보드에서도 **패닉하지 않고** 이유가 남는다.
    #[test]
    fn empty_capture_is_honest() {
        let none: [Representation; 0] = [];
        let c = capture(&none, None, None, None, &[], P);
        assert_eq!(c.kind, ClipKind::Text);
        assert_eq!(c.preview, Preview::None(PreviewMissing::NoRenderableRep));
        assert!(!c.preview.one_line().is_empty());
    }

    impl Captured {
        fn one_line_contains(&self, needle: &str) -> bool {
            self.preview.one_line().contains(needle)
        }
    }
}
