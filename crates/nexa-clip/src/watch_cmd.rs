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
//! | ★ **민감 표식** | 비밀번호 관리자에서 복사하면 **내용 없이 `(민감 표식 …)` 한 줄만** 찍힌다 |
//! | 출처 앱 | 복사한 앱 이름이 붙는가 |
//! | 용량 규칙 | 큰 항목에서 `버림 N KB`가 찍히는가 |

use nclip_core::capture::{capture, coalesces, is_password_manager_url, CapturePolicy};
use nclip_core::{ClipSnapshot, ClipboardWatch as _, WatchCapability};
use nclip_plat::watch::PlatformWatch;

/// ★ **연속 변화 합치기 창**(T-14g · D-80) — 이 시간 안에 "같은 복사의 다음 장면"
/// (동일 재게시 · 부분→완본)이 오면 이전 것을 버리고 나중 것으로 바꾼다.
///
/// 탐색기 복사 한 번이 2~4개 항목으로 쌓이던 것을 하나로 만든다(08-27 실기).
/// 값은 실측에서 왔다 — 재게시는 수십 ms, 부분→완본은 재시도(200ms) 직후에 온다.
const COALESCE_MS: u64 = 500;

/// 표시 정책 — 설정에서 한 번 읽어 온다(감시 도는 동안 고정).
#[derive(Clone, Copy)]
struct Gate {
    /// ★ D-79 — 브라우저 암호 관리자 복사를 출처 URL로 차단(기본 꺼짐 · 옵트인).
    conceal_browser_pw: bool,
}

impl Gate {
    fn load() -> Self {
        let s = crate::conf::Settings::load();
        Self {
            conceal_browser_pw: s.state.get("sec.conceal_browser_pw") == "on",
        }
    }
}

/// ★ **지금 클립보드만 한 번** 읽고 끝낸다(`peek`).
///
/// `watch`는 계속 떠 있어야 하는데, *"방금 복사한 게 뭐로 잡혔나"* 만 보고 싶을 때가 잦다.
/// 감시를 걸지 않으므로 **다른 `watch` 세션과 함께 써도 된다**.
pub(crate) fn peek() {
    let watch = PlatformWatch::new();
    match watch.capability() {
        WatchCapability::Supported { backend } => println!("클립보드: ok ({backend})"),
        WatchCapability::Unsupported { reason } => {
            eprintln!("클립보드를 읽을 수 없습니다: {reason:?}");
            std::process::exit(1);
        }
    }
    let gate = Gate::load();
    match watch.read_now() {
        Some(snap) => report(&snap, gate),
        // ⚠️ 빈 스냅숏으로 위장하지 않는다 — 못 연 것과 비어 있는 것은 다르다.
        None => eprintln!("클립보드를 열지 못했습니다(다른 앱이 잡고 있을 수 있습니다)."),
    }
}

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

    let gate = Gate::load();
    if gate.conceal_browser_pw {
        println!("브라우저 암호 차단: 켜짐 (sec.conceal_browser_pw)");
    }

    // ★ 켜자마자 지금 클립보드를 한 번 본다 — "복사해야만 뭔가 보이는" 상태를 피한다.
    if let Some(snap) = watch.read_now() {
        println!("\n[지금 클립보드]");
        report(&snap, gate);
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
    let mut emit = |snap: &ClipSnapshot| {
        n += 1;
        println!("[{n}]");
        report(snap, gate);
        println!();
    };

    // ★ 디바운스 수신 루프(D-80) — 스냅숏을 바로 찍지 않고 잠깐 들고 있다가,
    //   합칠 다음 장면이 오면 바꿔치우고, 조용해지면 그때 찍는다.
    //   ⚠️ 합쳐지지 **않는** 새 복사가 오면 들고 있던 것을 먼저 찍는다 — 순서는 지킨다.
    let window = std::time::Duration::from_millis(COALESCE_MS);
    let mut pending: Option<ClipSnapshot> = None;
    loop {
        let received = match &pending {
            // 들고 있는 게 있으면 창이 닫힐 때까지만 기다린다.
            Some(_) => match rx.recv_timeout(window) {
                Ok(s) => Some(s),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            },
            // 없으면 다음 변화까지 그냥 잔다(유휴 CPU 0).
            None => match rx.recv() {
                Ok(s) => Some(s),
                Err(_) => break,
            },
        };
        match received {
            Some(next) => {
                if let Some(prev) = pending.take() {
                    if !coalesces(&prev, &next) {
                        emit(&prev);
                    }
                    // 합쳐지면 prev는 버려진다 — 나중 것이 정본.
                }
                pending = Some(next);
            }
            // 창이 조용히 닫혔다 — 들고 있던 것을 찍는다.
            None => {
                if let Some(prev) = pending.take() {
                    emit(&prev);
                }
            }
        }
    }
    // 감시 스레드가 끊겨도 들고 있던 것은 잃지 않는다.
    if let Some(prev) = pending.take() {
        emit(&prev);
    }
}

/// 스냅숏 하나를 **캡처 파이프라인에 그대로 통과시켜** 사람이 읽게 찍는다.
///
/// ★ 진단용으로 따로 판정하지 않는다 — 실제 저장 경로와 **같은 함수**를 쓴다.
/// 그래야 여기서 맞게 보이면 제품에서도 맞다.
fn report(snap: &ClipSnapshot, gate: Gate) {
    // ★ 민감 표식(FR-S-1) — **내용을 읽지도 찍지도 않는다**. 다만 줄 자체는 남긴다:
    //   "막혀서 안 보인다"와 "이벤트를 놓쳤다"를 점검자가 구분할 수 있어야 한다.
    if snap.concealed {
        if let Some(app) = &snap.source_app {
            println!("  (민감 표식 — {app} 의 복사를 기록하지 않습니다)");
        } else {
            println!("  (민감 표식 — 기록하지 않습니다)");
        }
        return;
    }
    // ★ D-79 — 브라우저 암호 관리자는 표식을 안 붙인다(08-27 실기). 옵트인 시 출처 URL로 차단.
    if gate.conceal_browser_pw
        && snap
            .source_url()
            .is_some_and(|u| is_password_manager_url(&u))
    {
        println!("  (브라우저 암호 관리자 복사 — 기록하지 않습니다)");
        return;
    }
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
