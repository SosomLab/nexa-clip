//! `nexa-clip` — 본체. **조립 지점**이다.
//!
//! 여기서만 어댑터를 실체화해 [`nclip_core`]의 포트에 주입한다(의존성 역전).
//! 도메인 판단은 하지 않는다.
//!
//! ## 지금 상태 — 골격 점검 + K-1 스파이크
//!
//! 창·렌더는 T-12b2에서 붙는다. 지금 있는 것은 **환경 점검**과
//! ★ **K-1 스파이크**(포커스 복원 + 키 주입)다 — 이 왕복이 안 되면 제품이 성립하지 않으므로
//! 창보다 먼저 검증한다([docs/02 §7](../../docs/02-roadmap.md) · [docs/21](../../docs/21-manual-test.md)).

mod conf;
mod demo;
mod settings_win;

use nclip_core::{
    current_lang, tr, ClipboardWatch as _, Msg, PasteAs, PasteCapability, PasteInjector as _,
    WatchCapability,
};
use nclip_ctl::ViewMode;
use nclip_plat::paste::{spike_steal_focus, PlatformPaste};
use nclip_plat::watch::PlatformWatch;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("spike-paste") => spike_paste(&args[1..]),
        Some("demo") => demo::run(),
        Some("settings") => settings_win::run(),
        Some("--help" | "-h" | "help") => usage(),
        Some(other) => {
            eprintln!("알 수 없는 명령: {other}\n");
            usage();
            std::process::exit(2);
        }
        None => status(),
    }
}

fn usage() {
    println!(
        "\
nexa-clip [명령]

  (없음)         환경 점검 — 이 PC에서 무엇이 되고 무엇이 안 되는지
  demo           렌더 데모 — 창을 열고 S1 퀵 팝업 레이아웃을 그린다
                 (1/2/3 보기 모드 · T 테마 · Esc 종료)
  settings       설정 창 — 좌측 카테고리 + 검색 + 우측 폼(이식 프레임워크)
  spike-paste    K-1 스파이크 — 포커스 복원 + 붙여넣기 키 주입 검증
      --plain        평문 붙여넣기 경로로 시도
      --wait <초>    대상 앱을 고를 시간(기본 5)
  --help         이 도움말

점검 절차는 docs/21-manual-test.md 를 따른다."
    );
}

/// 환경 점검 — **되는 것과 안 되는 것을 정직하게** 보여준다.
fn status() {
    let lang = current_lang();
    println!("{} v{}", tr(lang, Msg::AppName), env!("CARGO_PKG_VERSION"));
    println!("target          : {}", std::env::consts::OS);

    let watch = PlatformWatch::new();
    match watch.capability() {
        WatchCapability::Supported { backend } => println!("clipboard watch : ok ({backend})"),
        WatchCapability::Unsupported { reason } => {
            println!("clipboard watch : unavailable ({reason:?})");
            println!(
                "                  → {}",
                tr(lang, Msg::StatusWatchUnsupported)
            );
        }
    }

    let paste = PlatformPaste::new();
    match paste.capability() {
        PasteCapability::Full { backend } => println!("paste inject    : ok ({backend})"),
        // ★ 권한 대기는 "안 됨"이 아니라 "켜면 됨"이다 — 구분해서 알린다.
        PasteCapability::NeedsPermission { backend, hint } => {
            println!("paste inject    : needs permission ({backend})");
            println!("                  → {hint}");
        }
        PasteCapability::ClipboardOnly { reason } => {
            println!("paste inject    : clipboard only ({reason:?})");
            println!("                  → 붙여넣기 키는 못 넣는다. 클립보드 적재까지만 한다");
        }
    }

    // ★ 설정 영속 — **저장된 값이 실제로 읽히는지**를 여기서 보인다(T-12c2).
    //
    //   ⚠️ 예전에는 `ViewMode::default()`를 찍었다 — 사용자가 설정에서 바꿔도
    //   이 줄은 영원히 `Compact`였다. **점검 화면이 거짓말을 하면 점검이 아니다.**
    let conf = conf::Settings::load();
    let saved = conf.path().exists();
    println!(
        "settings        : {} ({})",
        conf.path().display(),
        if saved {
            "저장본 사용"
        } else {
            "아직 없음 — 기본값"
        }
    );

    let view = ViewMode::from_code(conf.state.get("ui.view_mode")).unwrap_or_default();
    // `ViewMode`는 nclip-ctl에 있고 그쪽은 도메인(Msg)을 모른다 — 번역은 여기서 붙인다.
    let view_label = match view {
        ViewMode::Rich => Msg::ViewRich,
        ViewMode::Compact => Msg::ViewCompact,
        ViewMode::Plain => Msg::ViewPlain,
    };
    println!(
        "default view    : {} ({})",
        tr(lang, view_label),
        view.code()
    );
    println!("theme           : {}", conf.state.get("ui.theme"));
    println!("max items       : {}", conf.state.get("store.max_items"));
    println!("tray recent     : {}", conf.state.get("ui.tray_recent_n"));

    println!("sync            : {}", tr(lang, Msg::SyncEndToEnd));
    println!("status          : {}", tr(lang, Msg::StatusLocalOnly));
}

/// ★ K-1 스파이크 — 창 없이 **포커스 왕복**을 끝까지 검증한다.
///
/// 실물 흐름(단축키 → 팝업이 포커스 획득 → 선택 → 원래 창 복귀 → 주입)에서
/// 창만 빼고 그대로 재현한다. 창을 만들기 전에 이 왕복이 되는지 알아야
/// 나머지 설계가 의미를 갖는다.
fn spike_paste(args: &[String]) {
    let plain = args.iter().any(|a| a == "--plain");
    let wait_s: u64 = args
        .iter()
        .position(|a| a == "--wait")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let mut paste = PlatformPaste::new();

    println!("── K-1 스파이크: 포커스 복원 + 키 주입 ──");
    match paste.capability() {
        PasteCapability::Full { backend } => println!("[0] 능력      : ok ({backend})"),
        PasteCapability::NeedsPermission { backend, hint } => {
            println!("[0] 능력      : 권한 필요 ({backend})");
            println!("               → {hint}");
            println!("               권한을 켠 뒤 다시 실행하세요. 계속 진행하면 실패합니다.");
        }
        PasteCapability::ClipboardOnly { reason } => {
            println!("[0] 능력      : 주입 불가 ({reason:?}) — 이 타깃은 스파이크 대상이 아닙니다");
            std::process::exit(1);
        }
    }

    println!();
    println!("준비:");
    println!("  1) 아무 텍스트나 복사해 두세요(Ctrl+C / ⌘C).");
    println!("  2) 붙여넣을 앱(메모장·TextEdit 등)을 열고 커서를 두세요.");
    println!("  3) {wait_s}초 안에 그 앱을 클릭해 **포그라운드로** 두세요.");
    println!();
    for left in (1..=wait_s).rev() {
        print!("\r  대상 확정까지 {left}초… ");
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!("\r  대상 확정              ");

    // ② 팝업을 띄우기 "전"에 기억한다.
    if !paste.capture_focus() {
        println!("[2] 대상 기억 : 실패 — 포그라운드 창을 못 찾았습니다");
        std::process::exit(1);
    }
    let label = paste.target_label().unwrap_or_default();
    println!("[2] 대상 기억 : {label}");

    // ③ 팝업이 포커스를 뺏는 순간을 흉내 낸다(실물에서는 창이 뜨면서 일어난다).
    let stolen = spike_steal_focus();
    if stolen {
        println!("[3] 포커스 탈취: ok (우리에게 옴)");
    } else {
        // ★ 탈취가 실패하면 대상이 계속 포그라운드라, 복원이 "성공"해도 아무것도 안 한 것이다.
        println!("[3] 포커스 탈취: 실패");
        println!("     ⚠️ 대상이 계속 포그라운드로 남습니다 — 이 실행에서는");
        println!("        **복원 경로(AttachThreadInput)가 검증되지 않습니다.**");
        println!("        주입만 확인되며, 복원은 실제 팝업 창이 생긴 뒤 확인해야 합니다.");
    }
    std::thread::sleep(std::time::Duration::from_millis(600));

    // ⑤+⑥ 되돌리고 주입한다.
    let as_ = if plain {
        PasteAs::Plain
    } else {
        PasteAs::Original
    };
    match paste.restore_and_paste(as_) {
        Ok(()) => {
            println!("[5] 포커스 복원: ok");
            println!("[6] 키 주입    : ok ({as_:?})");
            println!();
            println!("✅ 대상 앱에 붙여넣기가 되었는지 확인하세요.");
            if !stolen {
                println!("   ⚠️ 단, [3]이 실패했으므로 **주입만 검증**된 것입니다(복원은 미검증).");
            }
            println!(
                "   되었으면 K-1 통과 · 안 되었으면 docs/21-manual-test.md 에 증상을 적으세요."
            );
        }
        Err(e) => {
            println!("[5/6] 실패     : {e:?}");
            println!();
            println!("❌ K-1 미통과. docs/21-manual-test.md 에 증상과 함께 기록하세요.");
            std::process::exit(1);
        }
    }
}
