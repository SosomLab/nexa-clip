//! `nclip-core` — 도메인 + 포트 + i18n.
//!
//! **허브 크레이트**: 다른 `nclip-*`에 의존하지 않는다(`cargo tree -p nclip-core` = 자기 자신뿐).
//! 어댑터(`nclip-plat`/`nclip-store`/`nclip-ui`)가 이쪽에 의존하고, 여기 선언된 **포트**를
//! 구현해 본체(`nexa-clip`)가 조립 시점에 주입한다 — 의존성 역전([docs/20 §5]).
//!
//! I/O를 하지 않는다. 네트워크·화면·파일 없이 테스트된다.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod capture;
pub mod diag;
pub mod history;
pub mod i18n;
pub mod img;
pub mod item;
pub mod paste;
pub mod ports;
/// ★ T-18d 1단 — 제한 리치텍스트 런 파서(09-03).
pub mod richtext;
pub mod search;

pub use capture::{
    capture, classify, classify_with_text, decode_plain, has_content, parse_hdrop, parse_uri_list,
    select_reps, CapturePolicy, Captured, Preview, PreviewMissing, RepInfo, ThumbInfo,
};
pub use diag::{DiagLog, Level, Record};
pub use i18n::{current_lang, set_lang, tr, Lang, Msg};
pub use item::{is_plain_format, ClipItem, ClipKind, ItemId, Representation};
pub use paste::{PasteAs, PasteCapability, PasteError, PasteInjector, PasteUnsupported};
pub use ports::{
    ClipSnapshot, ClipboardWatch, RawRep, UnsupportedReason, WatchCapability, WatchError,
};
