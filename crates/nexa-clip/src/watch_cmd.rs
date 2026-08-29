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

use nclip_core::capture::{
    app_in_list, capture, coalesces, parse_block_list, url_in_prefixes, CapturePolicy,
};
use nclip_core::{ClipSnapshot, ClipboardWatch as _, UnsupportedReason, WatchCapability};
use nclip_plat::watch::PlatformWatch;

/// ★ **연속 변화 합치기 창**(T-14g · D-80) — 이 시간 안에 "같은 복사의 다음 장면"
/// (동일 재게시 · 부분→완본)이 오면 이전 것을 버리고 나중 것으로 바꾼다.
///
/// 탐색기 복사 한 번이 2~4개 항목으로 쌓이던 것을 하나로 만든다(08-27 실기).
/// 값은 실측에서 왔다 — 재게시는 수십 ms, 부분→완본은 재시도(200ms) 직후에 온다.
const COALESCE_MS: u64 = 500;

/// 표시·수집 정책 — 설정에서 한 번 읽어 온다(감시 도는 동안 고정).
///
/// `watch`(진단)와 상주 셸([`crate::tray_cmd`])이 **같은 게이트**를 쓴다 —
/// 여기서 막힌 것은 이력에도 들어가지 않는다.
pub(crate) struct Gate {
    /// ★ D-79 — 브라우저 암호 관리자 복사를 출처 URL로 차단(기본 꺼짐 · 옵트인).
    conceal_browser_pw: bool,
    /// 차단할 출처 URL 접두 — ★ **사용자가 설정에서 직접 편집한다**(08-28 · 기본 = 코어 목록).
    conceal_urls: Vec<String>,
    /// 제외 앱(FR-S-2) — 여기 적힌 앱의 복사는 **토글과 무관하게** 기록하지 않는다(기본 없음).
    exclude_apps: Vec<String>,
}

impl Gate {
    pub(crate) fn load() -> Self {
        Self::from_state(&crate::conf::Settings::load())
    }

    /// 이미 열어 둔 설정에서 만든다(셸 — 설정을 두 번 열지 않는다).
    pub(crate) fn from_state(s: &crate::conf::Settings) -> Self {
        Self {
            conceal_browser_pw: s.state.get("sec.conceal_browser_pw") == "on",
            conceal_urls: parse_block_list(s.state.get("sec.conceal_urls")),
            exclude_apps: parse_block_list(s.state.get("sec.exclude_apps")),
        }
    }

    /// 이 스냅숏을 기록하면 안 되는가 — 막히면 **사유 한 줄**(내용 없음)을 준다.
    pub(crate) fn blocks(&self, snap: &ClipSnapshot) -> Option<String> {
        // ★ 민감 표식(FR-S-1) — 내용을 읽지도 남기지도 않는다.
        if snap.concealed {
            return Some(match &snap.source_app {
                Some(app) => format!("민감 표식 — {app} 의 복사를 기록하지 않습니다"),
                None => "민감 표식 — 기록하지 않습니다".into(),
            });
        }
        // ★ 제외 앱(FR-S-2) — 목록에 적은 앱은 토글과 무관하게 기록하지 않는다.
        if let Some(app) = &snap.source_app {
            if app_in_list(app, &self.exclude_apps) {
                return Some(format!("제외 앱 — {app} 의 복사를 기록하지 않습니다"));
            }
        }
        // ★ D-79 — 브라우저 암호 관리자는 표식을 안 붙인다(08-27 실기). 옵트인 시
        //   사용자가 편집한 출처 URL 목록으로 차단한다.
        if self.conceal_browser_pw
            && snap
                .source_url()
                .is_some_and(|u| url_in_prefixes(&u, self.conceal_urls.iter().map(String::as_str)))
        {
            return Some("브라우저 암호 관리자 복사 — 기록하지 않습니다".into());
        }
        None
    }
}

/// 못 쓰는 이유마다 **지금 할 수 있는 일**을 알려준다.
///
/// ⚠️ 예전에는 사유와 무관하게 *"이 OS의 감시 구현이 아직 없습니다"* 한 줄이었다 —
/// 도구만 설치하면 되는 Linux(`MissingTool`)를 **미구현으로 오인**하게 만든다(08-29).
/// 포트가 정직하게 사유를 돌려주는데 안내가 그걸 버리면 정직함이 사용자에게 닿지 않는다.
fn unsupported_hint(reason: &UnsupportedReason) -> String {
    match reason {
        UnsupportedReason::MissingTool(tool) => format!(
            "{tool} 이(가) 없습니다. 설치하세요 — Ubuntu/Debian `sudo apt install wl-clipboard xclip` · \
             Fedora/RHEL `sudo dnf install wl-clipboard xclip` · Arch `sudo pacman -S wl-clipboard xclip` · \
             openSUSE `sudo zypper install wl-clipboard xclip` · Alpine `doas apk add wl-clipboard xclip`."
        ),
        UnsupportedReason::NoDisplayServer => "표시 서버가 없습니다(헤드리스 · SSH 등). \
             데스크톱 세션에서 실행하거나 WAYLAND_DISPLAY/DISPLAY 를 넘겨 주세요."
            .into(),
        UnsupportedReason::WaylandNoDataControl => "이 Wayland 컴포지터에 data-control 프로토콜이 없습니다(GNOME 등). \
             wl-clipboard 폴백을 쓰거나 X11 세션으로 로그인하세요."
            .into(),
        UnsupportedReason::NotImplemented => {
            "이 OS의 감시 구현이 아직 없습니다. 진행 상황은 docs/21 참조.".into()
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
            eprintln!("  조치: {}", unsupported_hint(&reason));
            std::process::exit(1);
        }
    }
    let gate = Gate::load();
    match watch.read_now() {
        Some(snap) => report(&snap, &gate),
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
            eprintln!("  조치: {}", unsupported_hint(&reason));
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
        report(&snap, &gate);
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
        report(snap, &gate);
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
fn report(snap: &ClipSnapshot, gate: &Gate) {
    // ★ 게이트(민감 표식·제외 앱·브라우저 암호) — 막힌 것도 **줄은 남긴다**:
    //   "막혀서 안 보인다"와 "이벤트를 놓쳤다"를 점검자가 구분할 수 있어야 한다.
    if let Some(reason) = gate.blocks(snap) {
        println!("  ({reason})");
        return;
    }
    let plain = snap.plain_text();
    let policy = CapturePolicy::default();
    let names = snap.file_names();
    // ★ 이미지 치수(T-14c 1단) — 썸네일 원본 표현의 **머리글만** 읽는다(압축 해제 없음).
    //   blob_id는 저장소(T-16)가 생기기 전까지 0 — 목록 표시는 치수만 쓴다.
    let thumb = nclip_core::capture::thumbnail_source(&snap.reps).and_then(|i| {
        let r = &snap.reps[i];
        nclip_core::img::image_dimensions(&r.format, &r.data).map(|(w, h)| ([0u8; 32], w, h))
    });
    // 정제 HTML은 아직 없다(T-14d) — 없는 것을 있다고 하지 않는다.
    let c = capture(&snap.reps, plain.as_deref(), thumb, None, &names, policy);

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
