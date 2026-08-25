//! `nexa-clip` — 본체. **조립 지점**이다.
//!
//! 여기서만 어댑터를 실체화해 [`nclip_core`]의 포트에 주입한다(의존성 역전).
//! 도메인 판단은 하지 않는다.
//!
//! ## 지금 상태 — 골격 점검 CLI
//!
//! 창·렌더는 T-12b 이후에 붙는다([docs/13 §6](../../docs/13-ui-reuse-from-beep.md)).
//! 지금은 **조립이 실제로 되는지**와 **환경이 무엇을 지원하는지**를 눈으로 확인한다.

use nclip_core::{current_lang, tr, ClipboardWatch as _, Msg, WatchCapability};
use nclip_ctl::ViewMode;
use nclip_plat::watch::PlatformWatch;

fn main() {
    let lang = current_lang();
    println!("{} v{}", tr(lang, Msg::AppName), env!("CARGO_PKG_VERSION"));

    // 어댑터 조립 — 본체만이 실체를 안다.
    let watch = PlatformWatch::new();
    match watch.capability() {
        WatchCapability::Supported { backend } => {
            println!("clipboard watch : ok ({backend})");
        }
        // ★ 미지원을 조용히 넘기지 않는다(docs/02 R-4).
        WatchCapability::Unsupported { reason } => {
            println!("clipboard watch : unavailable ({reason:?})");
            println!("  → {}", tr(lang, Msg::StatusWatchUnsupported));
        }
    }

    let mode = ViewMode::default();
    println!(
        "default view    : {} ({})",
        tr(lang, Msg::ViewCompact),
        mode.code()
    );
    println!("status          : {}", tr(lang, Msg::StatusLocalOnly));
}
