// ★ Windows = `windows` 서브시스템(09-03 사용자 — 실행 때 콘솔 창이 먼저 뗴).
//   beep과 같은 원인(링커 기본값) · 같은 처방. 터미널 출력은 attach_parent로 살린다.
#![cfg_attr(windows, windows_subsystem = "windows")]
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

/// ★ 정보(About) — 버전·빌드·실행 파일 SHA-256(09-12).
/// 창 제목의 앱 이름 — ★ 디버그 빌드는 "(Debug)"를 붙인다(사용자 10-10 · nexa-dir3 10-06 규칙 동일 · 설치본과 debug 앱을 제목으로 구분).
/// 모든 창은 [`settings_win::win_name`]을 거치며 제목 앞머리 "Nexa Clip"이 이 값으로 바뀐다(릴리스 = 그대로).
pub(crate) const APP_TITLE: &str = if cfg!(debug_assertions) {
    "Nexa Clip (Debug)"
} else {
    "Nexa Clip"
};

mod about;
mod cliptext;
mod conf;
mod dedup;
mod demo;
mod devices;
mod icon;
mod keys;
mod kind_icon;
mod lan;
mod main_win;
mod mode_drop;
mod popup_win;
mod render_img;
mod rich_table;
mod search_index;
mod settings_win;
mod sync_cmd;
mod syncitem;
mod thumbs;
mod tray_cmd;
mod watch_cmd;
/// ★ 파일 내용 전송 관리자(09-12 · DR-30) — 캐시·당겨 받기·이어 받기·진행률.
mod xfer;

use nclip_core::{
    current_lang, tr, ClipboardWatch as _, Msg, PasteAs, PasteCapability, PasteInjector as _,
    WatchCapability,
};
use nclip_plat::paste::{spike_steal_focus, PlatformPaste};
use nclip_plat::watch::PlatformWatch;
use nexa_ctl::ViewMode;

fn main() {
    // 터미널에서 부르면 그 콘솔에 출력(windows 서브시스템 보완 · 09-03).
    nclip_plat::console::attach_parent();
    // ★ 이식 컨트롤(우클릭 편집 메뉴)의 라벨을 앱 i18n에 잇는다(미주입 기본 = 영어).
    nexa_ctl::controls::set_ctl_labels(|m| {
        use nexa_ctl::controls::CtlMsg as C;
        let lang = current_lang();
        match m {
            C::CtxSelectAll => tr(lang, Msg::CtxSelectAll),
            C::CtxCopy => tr(lang, Msg::CtxCopy),
            C::CtxCut => tr(lang, Msg::CtxCut),
            C::CtxPaste => tr(lang, Msg::CtxPaste),
        }
    });

    // ★ `--profile <이름>` / `--profile=<이름>`(09-04) — 어느 명령 앞뒤에 와도 되는 전역 옵션.
    let mut args: Vec<String> = Vec::new();
    let mut raw = std::env::args().skip(1);
    while let Some(a) = raw.next() {
        let name = if a == "--profile" {
            raw.next()
        } else {
            a.strip_prefix("--profile=").map(str::to_string)
        };
        match (name, a.starts_with("--profile")) {
            (Some(n), _) => {
                if !conf::valid_profile_name(&n) {
                    eprintln!("profile name must be 1-32 chars of letters, digits, - or _: {n:?}");
                    std::process::exit(2);
                }
                conf::set_profile(&n);
            }
            (None, true) => {
                eprintln!("--profile needs a name");
                std::process::exit(2);
            }
            (None, false) => args.push(a),
        }
    }
    if let Some(p) = conf::profile() {
        println!("profile: {p} — data {}", conf::data_dir().display());
    }
    // ★ `--license <status|request|install|remove|path>`(10-10 P4 · beep D-33-7 동일) — GUI와 같은 데이터 폴더.
    if args.first().map(String::as_str) == Some("--license") {
        let code = nclip_license::cli::run(
            &args[1..],
            &conf::data_dir(),
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        );
        std::process::exit(code);
    }
    match args.first().map(String::as_str) {
        Some("spike-paste") => spike_paste(&args[1..]),
        Some("demo") => demo::run(),
        Some("settings") => settings_win::run(),
        Some("watch") => watch_cmd::run(),
        Some("peek") => watch_cmd::peek(),
        Some("tray") => tray_cmd::run(),
        Some("status") => status(),
        // ★ 배포 검증용(brew formula test · 09-04) — 워크스페이스 버전을 그대로 찍는다.
        Some("--version" | "-V") => println!("nexa-clip {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h" | "help") => usage(),
        Some(other) => {
            eprintln!("unknown command: {other}\n");
            usage();
            std::process::exit(2);
        }
        // ★ 무인수 = 트레이 상주(09-03 사용자 — 더블클릭 기대 동작 · beep과 동일 계약).
        //   windows 서브시스템 전환으로 무인수 status는 보이지 않게 됐다 —
        //   환경 점검은 `status` 명시 명령으로.
        None => tray_cmd::run(),
    }
}

/// `--help` — ★ English only(10-10 user rule: anything shipped in a package — CLI help/status/version, release notes,
/// package descriptions — is written in English; the UI itself follows the system language via i18n).
fn usage() {
    println!(
        "\
nexa-clip [command]

  (none)         ★ Stay resident in the tray (= tray) — default double-click behaviour
  status         Environment check — what works on this PC and what does not
  demo           Render demo — opens a window with the S1 quick-popup layout
                 (1/2/3 view modes · T theme · Esc to quit)
  settings       Settings window — categories on the left, search, cards on the right
  watch          ★ Clipboard watch — prints what is captured on every copy
                 (kind detection · representation list · size rules · Ctrl+C to quit)
  peek           Read the clipboard once and exit (can be combined with watch)
  tray           ★ Tray residency — icon + right-click menu (Open/Quit) + watch
                 (capture count in the tooltip · quit from the menu)
  spike-paste    K-1 spike — verify focus restore + paste key injection
      --plain        try the plain-text paste path
      --wait <sec>   time to pick the target app (default 5)
  --license <status|request [name [email]]|install <file>|remove|path>
                 ★ License (10-10) — status · request code · install (verified only) · remove · install path
  --version      print the version (used by package tests)
  --help         this help
  --profile <name>  ★ Run with a separate profile (09-04) — data folder data/profiles/<name>
                 (separate settings · store · identity · device list · single-instance guard)
                 → test sync between two instances on one PC: `nexa-clip --profile b`

Manual test procedure: docs/21-manual-test.md"
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
            println!(
                "                  → cannot inject the paste key; the clipboard is loaded only"
            );
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
            "from saved settings"
        } else {
            "none yet — defaults"
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
    println!(
        "theme           : {} (OS: {})",
        conf.state.get("ui.theme"),
        match nclip_plat::theme::system_prefers_dark() {
            Some(true) => "dark",
            Some(false) => "light",
            None => "no preference",
        }
    );
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

    println!("── K-1 spike: focus restore + key injection ──");
    match paste.capability() {
        PasteCapability::Full { backend } => println!("[0] capability : ok ({backend})"),
        PasteCapability::NeedsPermission { backend, hint } => {
            println!("[0] capability : permission needed ({backend})");
            println!("               → {hint}");
            println!("               enable the permission and run again; continuing will fail.");
        }
        PasteCapability::ClipboardOnly { reason } => {
            println!("[0] capability : no injection ({reason:?}) — this target is out of scope for the spike");
            std::process::exit(1);
        }
    }

    println!();
    println!("Prepare:");
    println!("  1) copy any text (Ctrl+C / ⌘C).");
    println!("  2) open the target app (Notepad, TextEdit, …) and place the caret.");
    println!("  3) within {wait_s}s click that app so it is in the **foreground**.");
    println!();
    for left in (1..=wait_s).rev() {
        print!("\r  target locks in {left}s… ");
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!("\r  target locked           ");

    // ② 팝업을 띄우기 "전"에 기억한다.
    if !paste.capture_focus() {
        println!("[2] remember   : failed — no foreground window");
        std::process::exit(1);
    }
    let label = paste.target_label().unwrap_or_default();
    println!("[2] remember   : {label}");

    // ③ 팝업이 포커스를 뺏는 순간을 흉내 낸다(실물에서는 창이 뜨면서 일어난다).
    let stolen = spike_steal_focus();
    if stolen {
        println!("[3] steal focus: ok (we are foreground)");
    } else {
        // ★ 탈취가 실패하면 대상이 계속 포그라운드라, 복원이 "성공"해도 아무것도 안 한 것이다.
        println!("[3] steal focus: failed");
        println!("     ⚠️ the target stays in the foreground — in this run");
        println!("        **the restore path (AttachThreadInput) is NOT verified.**");
        println!(
            "        only injection is checked; verify restore once the real popup window exists."
        );
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
            println!("[5] restore    : ok");
            println!("[6] inject     : ok ({as_:?})");
            println!();
            println!("✅ check that the text was pasted into the target app.");
            if !stolen {
                println!(
                    "   ⚠️ [3] failed, so **only injection** is verified (restore unverified)."
                );
            }
            println!(
                "   pasted = K-1 passes · not pasted = record the symptom in docs/21-manual-test.md"
            );
        }
        Err(e) => {
            println!("[5/6] failed   : {e:?}");
            println!();
            println!("❌ K-1 failed. Record the symptom in docs/21-manual-test.md.");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod title_tests {
    /// 창 제목(사용자 10-10 · nexa-dir3 동일): 디버그 빌드만 "(Debug)" 꼬리 · 릴리스는 "Nexa Clip".
    #[test]
    fn app_title_marks_debug_builds() {
        assert_eq!(
            super::APP_TITLE.ends_with("(Debug)"),
            cfg!(debug_assertions)
        );
        assert!(super::APP_TITLE.starts_with("Nexa Clip"));
    }
}
