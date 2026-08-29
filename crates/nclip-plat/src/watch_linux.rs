//! Linux 클립보드 감시 1단 — **OS 기본 도구 파이프** (T-14 Linux 축).
//!
//! 외부 crate 0(DR-8) — beep `nbeep-plat/clipboard.rs`의 Linux 선례를 따라
//! `wl-paste`(Wayland · wl-clipboard) → `xclip`(X11) 사다리를 파이프로 쓴다.
//! 도구가 없으면 **정직하게** 없다고 말한다([`capability`]) — 조용히 빈 목록을
//! 돌려주지 않는다(docs/02 R-4).
//!
//! ## 왜 도구 파이프가 1단인가
//!
//! | 방식 | 상태 |
//! |---|---|
//! | X11 `XFixesSelectSelectionInput` | 본편(T-14 본체) — libX11/libXfixes 링크 결정 필요 |
//! | Wayland `zwlr/ext_data_control` | 본편 — ★ **GNOME 미제공** → 어차피 폴백이 필요하다 |
//! | **도구 파이프 + 폴링** | ✅ 1단 — 링크 의존 0 · 두 표시 서버 공통 · 실기 점검 가능 |
//!
//! ## 변화 감지 — 일련번호가 없다
//!
//! Windows(`GetClipboardSequenceNumber`)·macOS(`changeCount`)와 달리 Linux에는
//! 싼 변경 신호가 없다. 1단은 **내용 지문**(FNV)으로 감지한다 —
//! 틱마다 한 벌을 읽어 지문이 달라졌을 때만 스냅숏을 내보낸다.
//! ⚠️ 틱마다 읽는 비용이 있으므로 주기를 macOS보다 느슨하게 잡는다
//! (활동 500ms → 유휴 2s — DR-9). 본편(XFIXES/data-control)이 이 비용을 없앤다.
//!
//! ## 민감 표식
//!
//! KDE/Klipper 관례 — 타깃 `x-kde-passwordManagerHint`의 값이 `secret`이면
//! 기록 금지다. **값을 못 읽으면 금지로 본다**(fail-closed · FR-S-1).

use nclip_core::{ClipSnapshot, RawRep, UnsupportedReason, WatchCapability, WatchError};
use std::process::{Command, Stdio};

/// 변화가 있을 때 부를 것 — 스레드를 건너가므로 `Send`.
pub type Sink = Box<dyn Fn(ClipSnapshot) + Send>;

/// 활동 직후 폴링 주기 — 틱마다 실제로 읽으므로 macOS(200ms)보다 느슨하다.
const ACTIVE_MS: u64 = 500;
/// 유휴 상한 주기.
const IDLE_MAX_MS: u64 = 2000;
/// 이만큼 조용하면(틱 수) 주기를 늘리기 시작한다 — 500ms × 10 = 5초.
const IDLE_AFTER_TICKS: u32 = 10;
/// 유휴 진입 후 틱마다 늘리는 양.
const IDLE_STEP_MS: u64 = 250;

/// 한 표현에서 읽는 상한 — 지문 폴링이 초대형 항목으로 상주 예산을 깨지 않게(DR-9).
/// 캡처 정책의 용량 규칙과 별개로, **읽기 자체**의 안전판이다.
const MAX_REP_BYTES: usize = 64 * 1024 * 1024;

/// KDE/Klipper 민감 표식 타깃.
const KDE_PW_HINT: &str = "x-kde-passwordManagerHint";

// ───────────────────────────── 백엔드 판별

/// 어느 도구로 읽을지 — 표시 서버 환경 변수 + 도구 존재로 정한다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Backend {
    /// Wayland — `wl-paste`(wl-clipboard).
    Wayland,
    /// X11 — `xclip`.
    X11,
}

/// 도구가 PATH에 있는가 — 실행이 아니라 `--version` 한 번으로 확인한다.
fn tool_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// 환경을 보고 백엔드를 고른다. 못 고르면 **이유**를 준다.
fn pick_backend() -> Result<Backend, UnsupportedReason> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    if !wayland && !x11 {
        return Err(UnsupportedReason::NoDisplayServer);
    }
    // Wayland 세션이라도 XWayland(DISPLAY)가 같이 떠 있는 일이 흔하다 — Wayland를 먼저 본다.
    if wayland && tool_exists("wl-paste") {
        return Ok(Backend::Wayland);
    }
    if x11 && tool_exists("xclip") {
        return Ok(Backend::X11);
    }
    Err(UnsupportedReason::MissingTool(if wayland {
        "wl-clipboard (wl-paste)"
    } else {
        "xclip"
    }))
}

/// 이 환경의 감시 능력 — [`crate::watch::PlatformWatch`]가 그대로 내보낸다.
#[must_use]
pub fn capability() -> WatchCapability {
    match pick_backend() {
        Ok(Backend::Wayland) => WatchCapability::Supported {
            backend: "wayland-wl-paste",
        },
        Ok(Backend::X11) => WatchCapability::Supported {
            backend: "x11-xclip",
        },
        Err(reason) => WatchCapability::Unsupported { reason },
    }
}

// ───────────────────────────── 도구 실행

/// 명령을 실행해 표준 출력을 바이트로 받는다. 실패·비정상 종료는 `None`.
fn run_bytes(cmd: &str, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// 지금 클립보드가 내놓는 타깃(MIME/atom) 목록.
fn list_targets(backend: Backend) -> Option<Vec<String>> {
    let raw = match backend {
        Backend::Wayland => run_bytes("wl-paste", &["--list-types"])?,
        Backend::X11 => run_bytes("xclip", &["-selection", "clipboard", "-t", "TARGETS", "-o"])?,
    };
    Some(
        String::from_utf8_lossy(&raw)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// 타깃 하나의 날바이트.
fn read_target(backend: Backend, target: &str) -> Option<Vec<u8>> {
    match backend {
        Backend::Wayland => run_bytes("wl-paste", &["--no-newline", "--type", target]),
        Backend::X11 => run_bytes("xclip", &["-selection", "clipboard", "-t", target, "-o"]),
    }
}

// ───────────────────────────── 타깃 정리 (순수 — 테스트 대상)

/// X11 프로토콜의 **곁다리 타깃** — 내용이 아니라 셀렉션 기계 장치다. 읽지 않는다.
fn is_meta_target(t: &str) -> bool {
    matches!(
        t,
        "TARGETS"
            | "TIMESTAMP"
            | "MULTIPLE"
            | "SAVE_TARGETS"
            | "DELETE"
            | "INCR"
            | "COMPOUND_TEXT"
            | "CLIPBOARD_MANAGER"
    )
}

/// 타깃 이름을 **판정 어휘로 정규화**한다([docs/12](../../../docs/12-clipboard-formats.md)).
///
/// - X11 텍스트 atom(`UTF8_STRING`·`STRING`·`TEXT`)과 `text/plain;charset=utf-8` 류는
///   전부 `text/plain`으로 — [`nclip_core::capture`]가 아는 이름 하나로 모은다.
/// - 그 외 `;charset=…` 꼬리만 떼고 그대로 둔다 — **모르는 이름을 지어내지 않는다**
///   (아는 표준이 아니면 벤더로 세는 것이 캡처 규칙이다).
fn normalize_target(t: &str) -> String {
    let base = t.split(';').next().unwrap_or(t);
    match base {
        "UTF8_STRING" | "STRING" | "TEXT" | "text/plain" => "text/plain".into(),
        _ => base.to_string(),
    }
}

/// FNV-1a — 내용 지문(변화 감지 전용 · 보안 아님).
fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = if seed == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        seed
    };
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 스냅숏의 내용 지문 — 표현 이름·바이트·표식을 전부 섞는다.
fn fingerprint(snap: &ClipSnapshot) -> u64 {
    let mut h = fnv1a(0, &[u8::from(snap.concealed)]);
    for r in &snap.reps {
        h = fnv1a(h, r.format.as_bytes());
        h = fnv1a(h, &(r.data.len() as u64).to_le_bytes());
        h = fnv1a(h, &r.data);
    }
    h
}

// ───────────────────────────── 읽기

/// 지금 클립보드를 한 벌 읽는다. 도구·표시 서버가 없으면 `None`.
#[must_use]
pub fn read_snapshot() -> Option<ClipSnapshot> {
    let backend = pick_backend().ok()?;
    read_snapshot_with(backend)
}

fn read_snapshot_with(backend: Backend) -> Option<ClipSnapshot> {
    let targets = list_targets(backend)?;

    // ★ 민감 표식 먼저 — 표식이 서면 내용은 읽지 않는다(fail-closed).
    let concealed = targets.iter().any(|t| t == KDE_PW_HINT)
        && read_target(backend, KDE_PW_HINT)
            .is_none_or(|v| String::from_utf8_lossy(&v).trim() == "secret");

    let mut reps = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for t in &targets {
        if is_meta_target(t) || t == KDE_PW_HINT {
            continue;
        }
        let name = normalize_target(t);
        // 정규화로 겹친 이름(UTF8_STRING·STRING → text/plain)은 첫 것만 담는다.
        if seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        let data = if concealed {
            Vec::new()
        } else {
            // 못 읽은 타깃은 이름만 담는다(Windows 핸들 포맷과 같은 취급 — 분류는 이름만 본다).
            read_target(backend, t)
                .filter(|d| d.len() <= MAX_REP_BYTES)
                .unwrap_or_default()
        };
        reps.push(RawRep { format: name, data });
    }

    Some(ClipSnapshot {
        reps,
        // 도구 파이프로는 출처 앱을 알 수 없다 — 모르는 것을 지어내지 않는다.
        source_app: None,
        concealed,
        // Linux에는 OS 일련번호가 없다(0 = 모름 — ClipSnapshot 계약).
        seq: 0,
    })
}

// ───────────────────────────── 감시 루프

/// 감시를 켠다 — 전용 스레드에서 내용 지문을 적응형 주기로 폴링한다.
pub fn start(sink: Sink) -> Result<(), WatchError> {
    let backend = pick_backend().map_err(WatchError::Unsupported)?;
    std::thread::Builder::new()
        .name("nclip-watch-linux".into())
        .spawn(move || poll_loop(backend, &sink))
        .map_err(|e| WatchError::Os(format!("감시 스레드 생성 실패: {e}")))?;
    Ok(())
}

fn poll_loop(backend: Backend, sink: &Sink) {
    // 시작 시점의 내용은 "새 복사"가 아니다 — 지금 지문을 기준선으로 삼는다.
    let mut last = read_snapshot_with(backend).map(|s| fingerprint(&s));
    let mut idle_ticks: u32 = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(interval_ms(idle_ticks)));
        let Some(snap) = read_snapshot_with(backend) else {
            // 도구가 순간 실패했다(셀렉션 주인 교체 중 등) — 다음 틱에 다시.
            idle_ticks = 0;
            continue;
        };
        // ★ 결함 ⑫의 교훈(Windows 08-27): **내용 없는 스냅숏은 미처리** —
        //   비우고→채우는 틈을 읽었을 수 있다. 기준선을 건드리지 않고 다시 읽는다.
        //   표식(concealed)이 선 것은 이름만이어도 처리 대상이다 — 게이트가 버린다.
        if snap.reps.is_empty() && !snap.concealed {
            idle_ticks = 0;
            continue;
        }
        let fp = fingerprint(&snap);
        if last == Some(fp) {
            idle_ticks = idle_ticks.saturating_add(1);
            continue;
        }
        last = Some(fp);
        idle_ticks = 0;
        sink(snap);
    }
}

/// 적응형 주기 — 활동 500ms, 5초 조용하면 250ms씩 늘려 2s에서 멈춘다.
fn interval_ms(idle_ticks: u32) -> u64 {
    if idle_ticks < IDLE_AFTER_TICKS {
        ACTIVE_MS
    } else {
        // +1 — 유휴에 **진입한 첫 틱부터** 늘기 시작해야 한다(0이면 진입이 무의미하다).
        (ACTIVE_MS + (u64::from(idle_ticks - IDLE_AFTER_TICKS) + 1) * IDLE_STEP_MS).min(IDLE_MAX_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// X11 텍스트 atom들이 판정 어휘 하나(`text/plain`)로 모인다(docs/12).
    #[test]
    fn text_atoms_normalize_to_text_plain() {
        for t in ["UTF8_STRING", "STRING", "TEXT", "text/plain;charset=utf-8"] {
            assert_eq!(normalize_target(t), "text/plain", "{t}");
        }
        // 모르는 이름은 꼬리만 떼고 그대로 — 지어내지 않는다.
        assert_eq!(normalize_target("text/html;charset=utf-8"), "text/html");
        assert_eq!(normalize_target("image/png"), "image/png");
        assert_eq!(
            normalize_target("application/x-vnd.foo"),
            "application/x-vnd.foo"
        );
    }

    /// 셀렉션 기계 장치 타깃은 내용이 아니다 — 읽지 않는다.
    #[test]
    fn meta_targets_are_skipped() {
        for t in ["TARGETS", "TIMESTAMP", "MULTIPLE", "SAVE_TARGETS", "INCR"] {
            assert!(is_meta_target(t), "{t}");
        }
        assert!(!is_meta_target("text/plain"));
        assert!(!is_meta_target("image/png"));
    }

    /// 지문은 내용·이름·표식 어느 것이 달라져도 달라진다(변화 감지의 전부).
    #[test]
    fn fingerprint_tracks_content_name_and_marker() {
        let snap = |fmt: &str, data: &[u8], concealed: bool| ClipSnapshot {
            reps: vec![RawRep {
                format: fmt.into(),
                data: data.to_vec(),
            }],
            concealed,
            ..Default::default()
        };
        let a = fingerprint(&snap("text/plain", b"hello", false));
        assert_eq!(
            a,
            fingerprint(&snap("text/plain", b"hello", false)),
            "결정적"
        );
        assert_ne!(a, fingerprint(&snap("text/plain", b"hellp", false)), "내용");
        assert_ne!(a, fingerprint(&snap("text/html", b"hello", false)), "이름");
        assert_ne!(a, fingerprint(&snap("text/plain", b"hello", true)), "표식");
    }

    /// 적응형 주기는 활동 500ms → 유휴 2s 사이만 오간다(DR-9 — 틱마다 실제 읽기가 있다).
    #[test]
    fn interval_ramps_from_active_to_idle_cap() {
        assert_eq!(interval_ms(0), ACTIVE_MS);
        assert_eq!(interval_ms(IDLE_AFTER_TICKS - 1), ACTIVE_MS);
        assert!(interval_ms(IDLE_AFTER_TICKS) > ACTIVE_MS);
        assert_eq!(interval_ms(10_000), IDLE_MAX_MS);
        assert_eq!(interval_ms(u32::MAX), IDLE_MAX_MS);
    }
}
