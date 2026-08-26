//! `watch` — ★ **복사한 것이 실제로 잡히는지** 눈으로 보는 명령(T-14b).
//!
//! 창도 저장도 없다. 감시([`nclip_plat::watch`])와 캡처 판정([`nclip_core::capture`])을
//! 이어 붙여 **터미널에 한 줄씩 찍는다**. 목록 UI가 붙기 전에 파이프라인 전체를
//! 실기로 확인하는 자리다([docs/21](../../docs/21-manual-test.md)).
//!
//! ## 무엇이 확인되나
//!
//! | 항목 | 어떻게 |
//! |---|---|
//! | 감시가 실제로 도는가 | 복사할 때마다 줄이 늘어난다 |
//! | ★ **종류 판정** | PPT 도형이 `RichText`로, 파일이 `Files`로 찍히는가 |
//! | ★ **민감 표식** | 비밀번호 관리자에서 복사하면 **아무 줄도 안 늘어난다** |
//! | 출처 앱 | 복사한 앱 이름이 붙는가 |
//! | 용량 규칙 | 큰 항목에서 `버림 N KB`가 찍히는가 |

use nclip_core::capture::{capture, CapturePolicy};
use nclip_core::{ClipSnapshot, ClipboardWatch as _, WatchCapability};
use nclip_plat::watch::PlatformWatch;

/// 감시를 켜고 잡히는 것을 찍는다. `Ctrl+C`로 끝낸다.
pub(crate) fn run() {
    let mut watch = PlatformWatch::new();
    match watch.capability() {
        WatchCapability::Supported { backend } => {
            println!("클립보드 감시: ok ({backend})");
        }
        WatchCapability::Unsupported { reason } => {
            eprintln!("클립보드 감시를 쓸 수 없습니다: {reason:?}");
            eprintln!("  조치: 이 OS의 감시 구현이 아직 없습니다. 진행 상황은 docs/21 참조.");
            std::process::exit(1);
        }
    }

    // ★ 켜자마자 지금 클립보드를 한 번 본다 — "복사해야만 뭔가 보이는" 상태를 피한다.
    if let Some(snap) = watch.read_now() {
        println!("\n[지금 클립보드]");
        report(&snap);
    }

    println!("\n복사해 보세요. Ctrl+C 로 종료합니다.\n");

    let (tx, rx) = std::sync::mpsc::channel::<ClipSnapshot>();
    if let Err(e) = watch.start(Box::new(move |snap| {
        // ⚠️ 창 프로시저 안에서 오래 머물면 안 된다 — 받아서 넘기기만 한다.
        let _ = tx.send(snap);
    })) {
        eprintln!("감시 시작 실패: {e:?}");
        std::process::exit(1);
    }

    let mut n = 0u32;
    // 감시 스레드가 살아 있는 한 계속 받는다.
    while let Ok(snap) = rx.recv() {
        n += 1;
        println!("[{n}]");
        report(&snap);
        println!();
    }
}

/// 스냅숏 하나를 **캡처 파이프라인에 그대로 통과시켜** 사람이 읽게 찍는다.
///
/// ★ 진단용으로 따로 판정하지 않는다 — 실제 저장 경로와 **같은 함수**를 쓴다.
/// 그래야 여기서 맞게 보이면 제품에서도 맞다.
fn report(snap: &ClipSnapshot) {
    let plain = snap.plain_text();
    let policy = CapturePolicy::default();
    let names = snap.file_names();
    // 썸네일·정제 HTML은 아직 없다(T-14c·T-14d) — 없는 것을 있다고 하지 않는다.
    let c = capture(&snap.reps, plain.as_deref(), None, None, &names, policy);

    println!("  종류   : {:?}", c.kind);
    if let Some(app) = &snap.source_app {
        println!("  출처   : {app}");
    }
    println!("  표현   : {}개 (남김 {}개)", snap.reps.len(), c.keep.len());
    for (i, r) in snap.reps.iter().enumerate() {
        let size = if r.data.is_empty() {
            "(핸들)".to_string()
        } else {
            human(r.data.len() as u64)
        };
        // ★ 버려진 것을 눈에 보이게 — 조용히 사라지면 나중에 원인을 못 찾는다.
        let mark = if c.keep.contains(&i) { " " } else { "✂" };
        println!("         {mark} {:<42} {size}", r.format);
    }
    if c.dropped_bytes > 0 {
        println!("  버림   : {} (용량 상한)", human(c.dropped_bytes));
    }
    if c.over_budget {
        println!("  ⚠️ 필수 표현만으로도 상한을 넘었습니다 — 더 버리지 않았습니다");
    }
    println!("  미리보기: {}", c.preview.one_line());
}

/// 바이트를 사람이 읽는 크기로.
fn human(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_sizes_read_well() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(2048), "2.0 KB");
        assert_eq!(human(3 * 1024 * 1024), "3.0 MB");
    }
}
