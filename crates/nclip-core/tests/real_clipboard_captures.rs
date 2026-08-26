//! ★ **실기 회귀 박제** — 2026-08-27 `nexa-clip watch` 로 실제 잡은 18건.
//!
//! 설계만으로는 안 보이던 것이 첫 실행에서 넷, 두 번째 훑기에서 셋 더 나왔다
//! ([docs/27 §8-1](../../../docs/27-capture-cases.md)). 그 판정을 **여기 고정한다**.
//!
//! ## 이 파일이 있는 이유
//!
//! 분류 규칙은 앞으로도 손댈 일이 생긴다. 그때마다 *"PPT 도형은 어떻게 되지?"* 를
//! 다시 실기로 확인할 수는 없다. ★ **실제 클립보드가 준 이름 목록**을 그대로 두면
//! 규칙을 고칠 때 **무엇이 깨지는지 즉시 보인다**.
//!
//! ⚠️ 포맷 이름은 **실기 출력 그대로**다. 보기 좋게 고치지 말 것 —
//! 고치는 순간 이 파일은 실기 기록이 아니라 창작이 된다.

use nclip_core::capture::{classify_with_text, has_content};
use nclip_core::{ClipKind, RawRep};

/// 실기 한 건 — 이름 목록 + 평문 유무 + 기대 종류.
struct Case {
    /// `watch` 출력의 `[n]`.
    id: &'static str,
    /// 출처 앱.
    app: &'static str,
    formats: &'static [&'static str],
    /// 평문 표현의 내용(없었으면 `None`).
    plain: Option<&'static str>,
    expect: ClipKind,
}

/// PPT 도형·그림이 항상 함께 올리는 이미지 인코딩 묶음.
const PPT_IMAGES: &[&str] = &[
    "image/svg+xml",
    "PNG",
    "JFIF",
    "GIF",
    "CF_BITMAP",
    "CF_ENHMETAFILE",
    "CF_METAFILEPICT",
    "CF_DIB",
    "CF_DIBV5",
];

const CASES: &[Case] = &[
    // ── [1]·[4]·[6] PowerPoint 도형 — ★ 글이 없다 ──────────────────────
    Case {
        id: "[1] PPT 도형",
        app: "POWERPNT",
        formats: &[
            "DataObject",
            "Preferred DropEffect",
            "InShellDragLoop",
            "Object Descriptor",
            "PowerPoint 12.0 Internal Theme",
            "PowerPoint 12.0 Internal Color Scheme",
            "PowerPoint 12.0 Internal Shapes",
            "Art::GVML ClipFormat",
            "image/svg+xml",
            "PNG",
            "JFIF",
            "GIF",
            "CF_BITMAP",
            "CF_ENHMETAFILE",
            "CF_METAFILEPICT",
            "Ole Private Data",
            "CF_DIB",
            "CF_DIBV5",
        ],
        plain: None,
        // ★ 붙여넣으면 **편집 가능한 도형**이다 — 이미지도 서식 있는 글도 아니다.
        expect: ClipKind::Object,
    },
    // ── [2] Greenshot — 진짜 그림 ──────────────────────────────────────
    Case {
        id: "[2] Greenshot 스크린샷",
        app: "Greenshot",
        formats: &[
            "DataObject",
            "PNG",
            "CF_DIB",
            "Ole Private Data",
            "CF_BITMAP",
            "CF_DIBV5",
        ],
        plain: None,
        // 진짜 벤더 포맷이 없다 — 그림 인코딩뿐.
        expect: ClipKind::Image,
    },
    // ── [3] PowerPoint 표 — HTML이 있다 ────────────────────────────────
    Case {
        id: "[3] PPT 표",
        app: "POWERPNT",
        formats: &[
            "DataObject",
            "Object Descriptor",
            "PowerPoint 12.0 Internal Theme",
            "Art::Table ClipFormat",
            "HTML Format",
            "PowerPoint 12.0 Internal Shapes",
            "image/svg+xml",
            "PNG",
            "JFIF",
            "GIF",
            "Ole Private Data",
            "CF_DIB",
            "CF_DIBV5",
        ],
        plain: None,
        expect: ClipKind::RichText,
    },
    // ── [5] PowerPoint 글상자 — ★ 사용자가 처음 물었던 그 케이스 ────────
    Case {
        id: "[5] PPT 글상자",
        app: "POWERPNT",
        formats: &[
            "DataObject",
            "Art::Text ClipFormat",
            "HTML Format",
            "Rich Text Format",
            "CF_UNICODETEXT",
            "PNG",
            "JFIF",
            "GIF",
            "CF_BITMAP",
            "CF_ENHMETAFILE",
            "CF_METAFILEPICT",
            "Ole Private Data",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
            "CF_DIB",
            "CF_DIBV5",
        ],
        plain: Some("AOP 계획에 대해 MP 수행"),
        expect: ClipKind::RichText,
    },
    // ── [7]·[8]·[10] VS Code(Electron → Chromium 표식) ──────────────────
    Case {
        id: "[7] VS Code 문서",
        app: "Code",
        formats: &[
            "HTML Format",
            "CF_UNICODETEXT",
            "Chromium internal source RFH token",
            "Chromium internal source URL",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
        ],
        plain: Some("1. 화면 레이아웃 개괄"),
        expect: ClipKind::RichText,
    },
    // ── [9] rdpclip — ★ 자기 표식만 ────────────────────────────────────
    Case {
        id: "[9] rdpclip 표식뿐",
        app: "rdpclip",
        formats: &[
            "DataObject",
            "Terminal Services Private Data",
            "Ole Private Data",
        ],
        plain: None,
        // 종류는 나오지만 ★ **내용이 없어 항목이 되면 안 된다**(아래 별도 검사).
        expect: ClipKind::Text,
    },
    // ── [11] 평문 하나만 ───────────────────────────────────────────────
    Case {
        id: "[11] 평문",
        app: "(미상)",
        formats: &["CF_UNICODETEXT"],
        plain: Some("BOM 적용 기준"),
        expect: ClipKind::Text,
    },
    // ── [13] Excel 범위 — 표현 30개 ────────────────────────────────────
    Case {
        id: "[13] Excel 범위",
        app: "EXCEL",
        formats: &[
            "DataObject",
            "CF_ENHMETAFILE",
            "CF_METAFILEPICT",
            "CF_BITMAP",
            "Biff12",
            "Biff8",
            "Biff5",
            "CF_SYLK",
            "CF_DIF",
            "XML Spreadsheet",
            "HTML Format",
            "CF_UNICODETEXT",
            "CF_TEXT",
            "CSV",
            "Hyperlink",
            "Rich Text Format",
            "Embed Source",
            "Native",
            "OwnerLink",
            "Object Descriptor",
            "Link Source",
            "Link Source Descriptor",
            "Link",
            "CF_129",
            "ObjectLink",
            "Ole Private Data",
            "CF_LOCALE",
            "CF_OEMTEXT",
            "CF_DIB",
            "CF_DIBV5",
        ],
        plain: Some("수요계획 수립"),
        expect: ClipKind::RichText,
    },
    // ── [14] Excel — ★ 표현 0개(지연 렌더링 재채움 순간) ───────────────
    Case {
        id: "[14] Excel 빈 스냅숏",
        app: "EXCEL",
        formats: &[],
        plain: None,
        expect: ClipKind::Text,
    },
    // ── [15] Excel — RTF + 평문만 ──────────────────────────────────────
    Case {
        id: "[15] Excel(RTF)",
        app: "EXCEL",
        formats: &[
            "Rich Text Format",
            "CF_UNICODETEXT",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
        ],
        plain: Some("수요계획 수립"),
        expect: ClipKind::RichText,
    },
    // ── [16] Edge — ★ HTML 없이 평문만 + Chromium 표식 ─────────────────
    Case {
        id: "[16] Edge 평문",
        app: "msedge",
        formats: &[
            "CF_UNICODETEXT",
            "Chromium internal source RFH token",
            "Chromium internal source URL",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
        ],
        plain: Some("광주 공정 공통   창원 공정 공통"),
        // ★ 표식을 곁다리로 안 세면 **RichText로 잘못 분류된다**(실기에서 그랬다).
        expect: ClipKind::Text,
    },
    // ── [17] Edge — 웹 본문 ────────────────────────────────────────────
    Case {
        id: "[17] Edge 웹 본문",
        app: "msedge",
        formats: &[
            "HTML Format",
            "CF_UNICODETEXT",
            "Chromium internal source RFH token",
            "Chromium internal source URL",
            "CF_LOCALE",
            "CF_TEXT",
            "CF_OEMTEXT",
        ],
        plain: Some("SB_SNOP_DEV — 생산계획(FP) 모듈"),
        expect: ClipKind::RichText,
    },
    // ── [18] CopyQ가 되올린 것 ─────────────────────────────────────────
    Case {
        id: "[18] CopyQ 재게시",
        app: "copyq",
        formats: &[
            "DataObject",
            "HTML Format",
            "CF_UNICODETEXT",
            "CF_TEXT",
            "application/x-copyq-owner",
            "Ole Private Data",
            "CF_LOCALE",
            "CF_OEMTEXT",
        ],
        plain: Some("cargo run -p nexa-clip -- watch"),
        expect: ClipKind::RichText,
    },
];

fn reps(formats: &[&str]) -> Vec<RawRep> {
    formats
        .iter()
        .map(|f| RawRep {
            format: (*f).to_string(),
            data: vec![0; 8],
        })
        .collect()
}

/// ★ 실기 18건의 **종류 판정**을 고정한다.
#[test]
fn real_captures_classify_as_recorded() {
    for c in CASES {
        let got = classify_with_text(c.formats, c.plain);
        assert_eq!(
            got, c.expect,
            "{} ({}) — 기대 {:?}, 실제 {:?}",
            c.id, c.app, c.expect, got
        );
    }
}

/// ★ **내용이 없는 것은 항목이 아니다** — 실기 [9]·[14].
///
/// 종류 판정은 나오지만 저장하면 **목록에 빈 줄**이 쌓인다.
#[test]
fn empty_and_marker_only_captures_are_not_items() {
    for c in CASES {
        let has = has_content(&reps(c.formats));
        let expected = !matches!(c.id, "[9] rdpclip 표식뿐" | "[14] Excel 빈 스냅숏");
        assert_eq!(has, expected, "{} — 내용 유무 판정이 틀렸다", c.id);
    }
}

/// ★ PowerPoint 세 갈래가 **서로 다르게** 판정된다.
///
/// 도형(글 없음) · 표(HTML) · 글상자(평문) — 하나로 뭉뚱그리면 셋 다 틀린다.
#[test]
fn powerpoint_three_shapes_are_distinguished() {
    let by = |id: &str| CASES.iter().find(|c| c.id == id).expect(id);
    assert_eq!(
        classify_with_text(by("[1] PPT 도형").formats, None),
        ClipKind::Object
    );
    assert_eq!(
        classify_with_text(by("[3] PPT 표").formats, None),
        ClipKind::RichText
    );
    assert_eq!(
        classify_with_text(by("[5] PPT 글상자").formats, Some("글")),
        ClipKind::RichText
    );
}

/// ⚠️ PPT가 함께 올리는 이미지 인코딩이 **벤더로 세어지면 안 된다**.
#[test]
fn ppt_image_encodings_are_not_vendor() {
    use nclip_core::capture::{is_bitmap_format, is_vendor_format};
    for f in PPT_IMAGES {
        // 메타파일·비트맵 핸들은 별도 판정이라 여기선 벤더가 아니기만 하면 된다.
        assert!(
            !is_vendor_format(f),
            "{f}가 벤더로 세어졌다 — PPT 판정이 흔들린다"
        );
    }
    for f in ["PNG", "JFIF", "GIF", "image/svg+xml", "CF_DIB", "CF_DIBV5"] {
        assert!(is_bitmap_format(f), "{f}는 그림 포맷이다");
    }
}

/// Excel 표현 30개 중 **본문 격 포맷은 벤더로 남아야** 한다.
///
/// ⚠️ 곁다리를 넓게 잡다가 `Embed Source`·`Native` 를 버리면 **개체 붙여넣기가 깨진다**.
#[test]
fn excel_content_formats_stay_vendor() {
    use nclip_core::capture::{is_metadata_format, is_vendor_format};
    for f in ["Embed Source", "Native", "Biff12", "XML Spreadsheet"] {
        assert!(is_vendor_format(f), "{f}는 내용이다");
        assert!(!is_metadata_format(f), "{f}를 곁다리로 버리면 안 된다");
    }
    for f in [
        "OwnerLink",
        "ObjectLink",
        "Link Source",
        "Link Source Descriptor",
        "Object Descriptor",
        "Ole Private Data",
        "DataObject",
    ] {
        assert!(is_metadata_format(f), "{f}는 곁다리다");
    }
}
