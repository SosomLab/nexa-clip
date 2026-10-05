//! Linux 클립보드 감시 — ★ **본편 = [`crate::selection_x11`] 직접 구현**(x11rb+XFIXES · 09-02)
//! 이 1순위이고, 이 파일의 **도구 파이프 1단은 폴백 계단**으로 남는다(연결 실패·특수 환경).
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

// ★ 직접 구현(T-14 본편) — Linux 실빌드에서만. 다른 OS의 test 빌드(순수부 검증)는 스텁.
#[cfg(all(unix, not(target_os = "macos")))]
use crate::selection_x11 as native;
#[cfg(not(all(unix, not(target_os = "macos"))))]
mod native {
    pub(crate) fn available() -> bool {
        false
    }
    pub(crate) fn list_targets(_cap: usize) -> Option<Vec<String>> {
        None
    }
    pub(crate) fn read_target(_t: &str, _cap: usize) -> Option<Vec<u8>> {
        None
    }
    pub(crate) fn watch(_f: Box<dyn Fn(bool) + Send>) -> Result<(), String> {
        Err("스텁 — 이 타깃에는 없다".into())
    }
    pub(crate) fn owner_app() -> Option<String> {
        None
    }
}

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
///
/// ⚠️ 이 상한은 **읽으면서** 건다([`run_bytes`]) — 다 읽고 나서 버리면 이미 늦다.
/// 지문 폴링은 틱마다 전량을 읽으므로 이 값이 곧 순간 상주 메모리 상한이다.
const MAX_REP_BYTES: usize = 64 * 1024 * 1024;

/// 타깃 **목록**의 상한 — 이름 몇 줄이 이보다 클 이유가 없다(폭주 방어).
const MAX_TARGETS_BYTES: usize = 64 * 1024;

/// 변화를 본 뒤 **자리 잡았는지** 다시 보기까지의 간격.
const SETTLE_MS: u64 = 120;
/// 자리 잡기 재확인 상한 — 최악 지연 `SETTLE_MS × SETTLE_MAX`.
const SETTLE_MAX: u32 = 4;

/// KDE/Klipper 민감 표식 타깃.
const KDE_PW_HINT: &str = "x-kde-passwordManagerHint";

// ───────────────────────────── 백엔드 판별

/// 어느 도구로 읽을지 — 표시 서버 환경 변수 + 도구 존재로 정한다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Backend {
    /// Wayland — `wl-paste`(wl-clipboard).
    Wayland,
    /// ★ X11/XWayland — 직접 구현(x11rb + XFIXES · T-14 본편). 도구 불요.
    X11Native,
    /// X11 — `xclip`(폴백 계단).
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
    // ★ Wayland는 **data-control이 있을 때만** wl-paste(08-30 실기: 없으면 wl-clipboard가 매번
    //   숨은 창으로 포커스를 뺏는다 — GNOME이 그렇다). 없으면 XWayland(`xclip`) — Mutter가
    //   Wayland↔X11 셀렉션을 동기화하므로 포커스 없이 읽힌다(08-29 XWayland 7/7).
    let data_control = wayland && crate::wayland_probe::has_data_control();
    if data_control && tool_exists("wl-paste") {
        return Ok(Backend::Wayland);
    }
    // ★ 본편 — X11/XWayland면 직접 붙는다(도구·설치 불요). 연결 실패는 아래 계단으로.
    if x11 && native::available() {
        return Ok(Backend::X11Native);
    }
    if x11 && tool_exists("xclip") {
        return Ok(Backend::X11);
    }
    if wayland && !data_control && !x11 {
        // 순수 Wayland + data-control 없음 = 포커스 없이 읽을 길이 없다.
        // ★ 전용 사유로(10-05 · T-41 ⑪) — 종전에는 "xclip 없음"으로 알려, 깔아도 안 되는 것을 깔라고 했다.
        return Err(UnsupportedReason::WaylandNoDataControl);
    }
    Err(UnsupportedReason::MissingTool(if data_control {
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
        Ok(Backend::X11Native) => WatchCapability::Supported {
            backend: if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                "xwayland-x11rb"
            } else {
                "x11-x11rb"
            },
        },
        Ok(Backend::X11) => WatchCapability::Supported {
            backend: if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                "xwayland-xclip"
            } else {
                "x11-xclip"
            },
        },
        Err(reason) => WatchCapability::Unsupported { reason },
    }
}

// ───────────────────────────── 도구 실행

/// 명령을 실행해 표준 출력을 바이트로 받는다 — ★ **`cap`까지만 읽는다**.
///
/// 실패·비정상 종료·**상한 초과**는 전부 `None`이다. 상한을 넘긴 표현은 호출자가
/// **이름만** 담는다(Windows 핸들 포맷과 같은 취급).
///
/// ⚠️ `Command::output()`을 쓰지 않는 이유 — 그건 전량을 메모리에 담은 **뒤에야**
/// 크기를 알 수 있다. 1 GB 이미지가 클립보드에 있으면 틱마다 1 GB를 담게 된다(DR-9).
fn run_bytes(cmd: &str, args: &[&str], cap: usize) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut buf = Vec::new();
    let read_ok = {
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        // cap + 1 — 넘겼다는 **사실**은 알아야 하되 전량을 담지는 않는다.
        // 블록이 끝나며 파이프가 닫힌다(열어 둔 채 wait 하면 교착이다).
        stdout.take(cap as u64 + 1).read_to_end(&mut buf).is_ok()
    };
    if !read_ok || buf.len() > cap {
        if read_ok {
            diag(&format!("{cmd}: 상한 {cap}B 초과 — 이름만 담는다"));
        }
        // 남은 출력을 계속 받아 줄 이유가 없다.
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    child.wait().ok()?.success().then_some(buf)
}

/// 지금 클립보드가 내놓는 타깃(MIME/atom) 목록.
fn list_targets(backend: Backend) -> Option<Vec<String>> {
    let raw = match backend {
        Backend::X11Native => return native::list_targets(MAX_TARGETS_BYTES),
        Backend::Wayland => run_bytes("wl-paste", &["--list-types"], MAX_TARGETS_BYTES)?,
        Backend::X11 => run_bytes(
            "xclip",
            &["-selection", "clipboard", "-t", "TARGETS", "-o"],
            MAX_TARGETS_BYTES,
        )?,
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
        Backend::X11Native => native::read_target(target, MAX_REP_BYTES),
        Backend::Wayland => run_bytes(
            "wl-paste",
            &["--no-newline", "--type", target],
            MAX_REP_BYTES,
        ),
        Backend::X11 => run_bytes(
            "xclip",
            &["-selection", "clipboard", "-t", target, "-o"],
            MAX_REP_BYTES,
        ),
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

/// 텍스트 계열 타깃의 **신뢰 순위**(낮을수록 먼저) — `None`이면 텍스트 계열이 아니다.
///
/// TARGETS 순서를 믿지 않는다: Mutter XWayland 브리지는 charset 없는 `text/plain`을 선두에
/// 두는데, GTK/glib은 그 타깃을 **ASCII**로 변환하며 비ASCII를 `\uXXXX`(Firefox 한글) ·
/// `\E2\9E\9C`(VTE 프롬프트 화살표)로 이스케이프한다(09-03 실기). `STRING`은 Latin-1.
/// UTF-8이 보장되는 `text/plain;charset=utf-8` · `UTF8_STRING`을 먼저 집는다.
fn text_rank(t: &str) -> Option<u8> {
    let base = t.split(';').next().unwrap_or(t);
    let utf8 = t.to_ascii_lowercase().contains("charset=utf-8");
    match base {
        "text/plain" if utf8 => Some(0),
        "UTF8_STRING" => Some(1),
        "text/plain" => Some(2),
        "TEXT" => Some(3),
        "STRING" => Some(4),
        _ => None,
    }
}

/// 읽기 순서 — 텍스트 계열은 [`text_rank`] 순, 나머지는 TARGETS 순서 그대로.
fn read_order(targets: &[String]) -> Vec<&String> {
    let mut text: Vec<&String> = targets.iter().filter(|t| text_rank(t).is_some()).collect();
    text.sort_by_key(|t| text_rank(t));
    let others = targets.iter().filter(|t| text_rank(t).is_none());
    text.into_iter().chain(others).collect()
}

/// 가상 머신 클립보드 다리가 **지연 전송 파일**에 붙이는 임시 경로의 표식(10-05 · T-70).
/// VMware Tools(`vmtoolsd -n vmusr`) — `/var/run/vmblock-fuse/blockdir/<임시>/…`(FUSE) ·
/// `/proc/fs/vmblock/mountPoint/<임시>/…`(구형 커널 모듈).
const VM_STAGING_MARKS: [&[u8]; 2] = [b"/vmblock-fuse/blockdir/", b"/vmblock/mountPoint/"];

/// Nautilus 레거시 표식 — 평문 **내용**의 첫 줄([`nclip_core::capture::promote_nautilus_text`]와 같은 이름).
const NAUTILUS_TEXT_MARK: &str = "x-special/nautilus-clipboard";

/// 이 표현이 **파일 목록**인가 — 파일 계열 이름이거나, 첫 줄이 Nautilus 표식인 평문(VMware 다리의 변종).
/// `format`은 정규화한 이름([`normalize_target`]).
fn is_file_listing(format: &str, data: &[u8]) -> bool {
    nclip_core::capture::is_files_format(format)
        || (format == "text/plain"
            && data
                .split(|b| *b == b'\n')
                .next()
                .is_some_and(|l| l.trim_ascii() == NAUTILUS_TEXT_MARK.as_bytes()))
}

/// 내용에 가상 머신 다리의 임시 경로가 들어 있는가([`VM_STAGING_MARKS`]).
fn is_vm_staging(data: &[u8]) -> bool {
    VM_STAGING_MARKS
        .iter()
        .any(|m| data.windows(m.len()).any(|w| w == *m))
}

/// 클립보드 소유자가 가상 머신 다리인가 — VMware는 프로세스 이름 `vmtoolsd` · 창 클래스 `Vmware-user`.
fn is_vm_bridge_owner(app: &str) -> bool {
    let lower = app.to_ascii_lowercase();
    lower == "vmtoolsd" || lower.contains("vmware")
}

/// 타깃 목록이 스냅숏에 남길 **표현 이름**(정규화 · 곁다리 제외 · 겹침 제거 · 정렬) —
/// 내용을 읽지 않고 "표현 구성이 그대로인가"를 견주는 데 쓴다([`settle`]).
fn rep_names(targets: &[String]) -> Vec<String> {
    let mut names: Vec<String> = targets
        .iter()
        .filter(|t| !is_meta_target(t) && t.as_str() != KDE_PW_HINT)
        .map(|t| normalize_target(t))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 타깃 이름만의 지문 — 내용은 읽지 않는다.
fn names_hash(targets: &[String]) -> u64 {
    let mut h = fnv1a(0, &[]);
    for t in targets {
        h = fnv1a(h, t.as_bytes());
        h = fnv1a(h, &[0]);
    }
    h
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

/// ★ 지금 클립보드에 **글이 있는가** — 표현 목록만 본다(10-05 · T-41 ④). 우클릭 메뉴의 "붙여넣기"
/// 활성 판정이 내용을 전부 읽어 오던 것(표현마다 변환 · 큰 그림이면 수십 MB)을 대신한다.
#[must_use]
pub fn has_text() -> bool {
    pick_backend()
        .ok()
        .and_then(list_targets)
        .is_some_and(|t| t.iter().any(|n| text_rank(n).is_some()))
}

/// 지금 클립보드를 한 벌 읽는다. 도구·표시 서버가 없으면 `None`.
#[must_use]
pub fn read_snapshot() -> Option<ClipSnapshot> {
    let backend = pick_backend().ok()?;
    read_snapshot_with(backend)
}

fn read_snapshot_with(backend: Backend) -> Option<ClipSnapshot> {
    match read_full(backend) {
        Read::Snap(s) => Some(s),
        Read::VmLazy | Read::Unreadable => None,
    }
}

/// 한 벌 읽기의 결과.
enum Read {
    Snap(ClipSnapshot),
    /// ★ 가상 머신 다리의 **지연 전송 파일**(10-05 · T-70) — 항목이 아니다. 더 읽지 않는다.
    VmLazy,
    /// 도구·소유자가 답하지 않았거나 클립보드가 비었다.
    Unreadable,
}

/// ★ 가상 머신 다리의 파일은 **요청할 때마다 실제 전송이 시작된다**(10-05 · T-70 · 사용자 실기) —
/// VMware 다리는 파일 타깃을 요청받으면 그때마다 새 임시 폴더를 만들어 호스트에서 파일을 끌어오고
/// (게스트에 복사 창이 뜬다) 그 임시 경로를 돌려준다. 종전에는 한 번의 복사에 표현 전부 ×
/// 자리 잡기 재읽기로 20번쯤 요청해 같은 파일이 20벌 넘게 전송됐다(경로가 매번 달라 지문도 안 맞았다).
/// 그 경로는 전송용 임시 자리라 이력에 남길 것도 아니다 → 다리의 파일은 **읽지 않고 버린다**:
/// 소유자가 다리이고 파일 타깃이 보이면 요청 0회, 소유자를 모르면 첫 파일 목록에서 임시 경로를 보고 멈춘다.
fn read_full(backend: Backend) -> Read {
    let Some(targets) = list_targets(backend) else {
        return Read::Unreadable;
    };
    diag(&format!("TARGETS(원시 순서): {targets:?}"));

    // ★ 내재 X11 경로는 소유자 창에서 앱 이름을 읽는다(10-05 · T-41 ⑥). 도구 파이프(wl-paste·xclip)로는
    //   알 수 없다 — 모르는 것을 지어내지 않는다.
    let source_app = (backend == Backend::X11Native)
        .then(native::owner_app)
        .flatten();
    if source_app.as_deref().is_some_and(is_vm_bridge_owner)
        && targets
            .iter()
            .any(|t| nclip_core::capture::is_files_format(&normalize_target(t)))
    {
        diag("가상 머신 다리의 파일 타깃 — 읽지 않는다(요청하면 전송이 시작된다)");
        return Read::VmLazy;
    }

    // ★ 민감 표식 먼저 — 표식이 서면 내용은 읽지 않는다(fail-closed).
    let concealed = targets.iter().any(|t| t == KDE_PW_HINT)
        && read_target(backend, KDE_PW_HINT)
            .is_none_or(|v| String::from_utf8_lossy(&v).trim() == "secret");

    let mut reps = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let ordered = read_order(&targets);
    let mut text_left = ordered.iter().filter(|t| text_rank(t).is_some()).count();
    for t in ordered {
        if is_meta_target(t) || t == KDE_PW_HINT {
            continue;
        }
        let is_text = text_rank(t).is_some();
        if is_text {
            text_left -= 1;
        }
        let name = normalize_target(t);
        // 정규화로 겹친 이름(UTF8_STRING·STRING → text/plain)은 **순위 첫 것**만 담는다.
        if seen.contains(&name) {
            continue;
        }
        let data = if concealed {
            Vec::new()
        } else {
            match read_target(backend, t) {
                Some(d) => d,
                // 텍스트 계열은 다음 순위로 물러난다(UTF8_STRING이 지연 제공이면 text/plain).
                None if is_text && text_left > 0 => {
                    diag(&format!("{t}: 읽기 실패 — 다음 순위 텍스트 타깃으로"));
                    continue;
                }
                // 못 읽은 타깃(지연 제공·상한 초과)은 이름만 담는다
                // — Windows 핸들 포맷과 같은 취급이다(분류는 이름만 본다).
                None => Vec::new(),
            }
        };
        if is_file_listing(&name, &data) && is_vm_staging(&data) {
            diag("가상 머신 다리의 임시 경로 — 나머지 타깃을 읽지 않고 버린다");
            return Read::VmLazy;
        }
        seen.push(name.clone());
        reps.push(RawRep { format: name, data });
    }

    Read::Snap(ClipSnapshot {
        reps,
        source_app,
        concealed,
        // Linux에는 OS 일련번호가 없다(0 = 모름 — ClipSnapshot 계약).
        seq: 0,
    })
}

// ───────────────────────────── 감시 루프

/// `NEXA_CLIP_DIAG=1`일 때만 stderr로 진단을 찍는다(watch_win·watch_mac과 동일 규약).
fn diag(msg: &str) {
    if std::env::var_os("NEXA_CLIP_DIAG").is_some() {
        eprintln!("[diag] {msg}");
    }
}

/// 감시를 켠다 — 전용 스레드에서 내용 지문을 적응형 주기로 폴링한다.
pub fn start(sink: Sink) -> Result<(), WatchError> {
    let backend = pick_backend().map_err(WatchError::Unsupported)?;
    diag(&format!("Linux 감시 시작 — 백엔드 {backend:?}"));
    if backend == Backend::X11Native {
        return native_event_loop(sink);
    }
    std::thread::Builder::new()
        .name("nclip-watch-linux".into())
        .spawn(move || poll_loop(backend, &sink))
        .map_err(|e| WatchError::Os(format!("감시 스레드 생성 실패: {e}")))?;
    Ok(())
}

/// ★ 본편 감시 — XFIXES 이벤트가 깨우면 한 벌 읽는다. **유휴 비용 0**(폴링·spawn 소멸 · DR-9).
///
/// 몰린 이벤트(연속 복사)는 채널을 비워 한 번으로 합치고, [`settle`]이 부분 스냅숏을
/// 다잡는다(08-29 결함 재발 방지 — 이벤트 직후에도 표현이 덜 올라온 순간이 있다).
/// 지문 중복 제거는 유지한다 — 내용이 같은 소유자 교체(재게시)를 걸러 준다.
/// 같은 내용의 재복사를 다시 넘기기까지의 최소 간격 — 한 번의 복사에 소유권을 여러 번 잡는 앱을 거른다.
const RECOPY_GAP: std::time::Duration = std::time::Duration::from_secs(1);

fn native_event_loop(sink: Sink) -> Result<(), WatchError> {
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    native::watch(Box::new(move |copied| {
        let _ = tx.send(copied);
    }))
    .map_err(WatchError::Os)?;
    std::thread::Builder::new()
        .name("nclip-watch-x11rb".into())
        .spawn(move || {
            // 시작 시점의 내용은 "새 복사"가 아니다 — 기준선.
            let mut last = read_snapshot_with(Backend::X11Native).map(|s| fingerprint(&s));
            let mut delivered = std::time::Instant::now();
            // 마지막으로 넘긴 스냅숏의 출처 앱 — 재복사 판정에 쓴다.
            let mut last_src: Option<String> = None;
            while let Ok(mut copied) = rx.recv() {
                // 몰린 이벤트 합치기 — 하나라도 "다른 앱이 복사했다"면 복사다.
                while let Ok(c) = rx.try_recv() {
                    copied |= c;
                }
                let Some(snap) = read_snapshot_with(Backend::X11Native) else {
                    continue;
                };
                if snap.reps.is_empty() && !snap.concealed {
                    continue;
                }
                let fp = fingerprint(&snap);
                if last == Some(fp) {
                    // ★ 같은 내용을 **다시 복사**했다(10-05 · T-41 ⑤) — Windows·mac은 일련번호가 올라 이력이
                    //   그 항목을 맨 위로 올린다(복사 수 +1). Linux는 지문이 같으면 버려서 안 올라왔다.
                    //   다른 앱이 소유권을 새로 잡은 이벤트면 그대로 넘긴다(이력이 승격으로 처리).
                    //   같은 복사에 소유권을 두세 번 잡는 앱이 있어 직전 전달 뒤 1초는 종전대로 버린다.
                    //   ★ **같은 앱**이 다시 잡았을 때만이다(실기 10-05) — 복사한 앱이 끝나면 클립보드 관리자나
                    //   가상 머신 다리가 같은 내용으로 소유권을 넘겨받는데, 그것은 재복사가 아니다(복사 수가 부푼다).
                    if copied
                        && delivered.elapsed() >= RECOPY_GAP
                        && snap.source_app.is_some()
                        && snap.source_app == last_src
                    {
                        delivered = std::time::Instant::now();
                        diag("같은 내용 재복사 — 승격으로 넘김");
                        sink(snap);
                    }
                    continue;
                }
                let (snap, fp) = settle(Backend::X11Native, snap, fp);
                last = Some(fp);
                last_src.clone_from(&snap.source_app);
                delivered = std::time::Instant::now();
                diag(&format!(
                    "변화 감지(x11rb) — 표현 {}개 · 표식 {}",
                    snap.reps.len(),
                    snap.concealed
                ));
                sink(snap);
            }
        })
        .map_err(|e| WatchError::Os(format!("감시 스레드 생성 실패: {e}")))?;
    Ok(())
}

/// ★ 가벼운 변화 탐지에 쓸 **대표 타깃 하나**(10-05 · T-54) — 글이 있으면 가장 믿을 만한 글 타깃,
/// 없으면 곁다리가 아닌 첫 타깃. 타깃이 하나도 없으면 `None`.
fn probe_target(targets: &[String]) -> Option<&String> {
    targets
        .iter()
        .filter_map(|t| text_rank(t).map(|k| (k, t)))
        .min_by_key(|(k, _)| *k)
        .map(|(_, t)| t)
        .or_else(|| {
            targets
                .iter()
                .find(|t| !is_meta_target(t) && t.as_str() != KDE_PW_HINT)
        })
}

/// ★ 가벼운 지문(10-05 · T-54) — **타깃 목록 + 대표 타깃 하나의 내용**만으로 만든다(도구 실행 2회).
///
/// 도구 파이프 폴링은 종전에 매 틱 타깃 목록 1회 + **표현마다 1회**씩 도구를 띄웠다(Office·그림 복사는
/// 표현이 5~18개 → 틱당 6~19회 · 하루 수십만 회). 보안 프로그램(EDR)에는 수상한 반복 실행이고 전력도 쓴다.
/// 변화가 없는 대부분의 틱은 이 지문이 같으므로 거기서 끝내고, 달라졌을 때만 전부 읽는다.
/// 한계: 타깃 목록과 대표 타깃이 그대로인 채 **다른 표현만** 바뀐 복사는 다음 변화까지 못 본다(드물다).
///
/// 둘째 값 = 대표 타깃이 **가상 머신 다리의 임시 경로 파일 목록**이었다(T-70) — 호출자는 타깃 목록이
/// 바뀔 때까지 내용을 다시 읽지 않는다(읽을 때마다 전송이 시작된다).
fn quick_fingerprint(backend: Backend, targets: &[String]) -> (u64, bool) {
    let mut h = names_hash(targets);
    let mut vm_lazy = false;
    if let Some(t) = probe_target(targets) {
        let data = read_target(backend, t).unwrap_or_default();
        vm_lazy = is_file_listing(&normalize_target(t), &data) && is_vm_staging(&data);
        h = fnv1a(h, &(data.len() as u64).to_le_bytes());
        h = fnv1a(h, &data);
    }
    (h, vm_lazy)
}

fn poll_loop(backend: Backend, sink: &Sink) {
    // 시작 시점의 내용은 "새 복사"가 아니다 — 지금 지문을 기준선으로 삼는다.
    let mut last = read_snapshot_with(backend).map(|s| fingerprint(&s));
    let mut last_quick: Option<u64> = None;
    // ★ 가상 머신 다리의 지연 전송 파일이 올라와 있는 동안의 타깃 이름 지문(T-70) — 같은 동안은 내용을
    //   읽지 않는다(읽을 때마다 호스트에서 파일 전송이 시작된다).
    let mut vm_lazy: Option<u64> = None;
    if let Some(t) = list_targets(backend) {
        let (q, lazy) = quick_fingerprint(backend, &t);
        last_quick = Some(q);
        vm_lazy = lazy.then(|| names_hash(&t));
    }
    let mut idle_ticks: u32 = 0;
    // 읽기 실패가 이어질 때 진단을 도배하지 않기 위한 상태(전이에서만 찍는다).
    let mut was_unreadable = false;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(interval_ms(idle_ticks)));
        // ★ 가벼운 지문이 그대로면 전부 읽지 않는다(T-54) — 변화 없는 틱의 도구 실행을 1+N회에서 2회로.
        let targets = list_targets(backend);
        let names = targets.as_deref().map(names_hash);
        if vm_lazy.is_some() && vm_lazy == names {
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        }
        vm_lazy = None;
        let quick = targets.as_deref().map(|t| quick_fingerprint(backend, t));
        if let Some((_, true)) = quick {
            diag("가상 머신 다리의 임시 경로 — 타깃이 바뀔 때까지 읽지 않는다");
            vm_lazy = names;
            last_quick = quick.map(|(q, _)| q);
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        }
        let quick = quick.map(|(q, _)| q);
        if quick.is_some() && quick == last_quick {
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        }
        last_quick = quick;
        let snap = match read_full(backend) {
            Read::Snap(s) => Some(s),
            Read::VmLazy => {
                vm_lazy = names;
                idle_ticks = next_idle_ticks(idle_ticks, false);
                continue;
            }
            Read::Unreadable => None,
        };
        let Some(snap) = snap else {
            // 도구가 순간 실패했다(셀렉션 주인 교체 중) **또는 클립보드가 비었다**
            // — `wl-paste --list-types`는 빈 클립보드에서 비정상 종료한다.
            //
            // ⚠️ 여기서 `idle_ticks = 0`으로 되돌리면 **빈 클립보드가 영구히 활동
            //    주기로 돈다**(500ms마다 프로세스 생성 — 로그인 직후처럼 오래 가는
            //    정상 상태다). 변화가 없었으니 유휴로 물러나는 것이 맞다(DR-9).
            if !was_unreadable {
                diag("클립보드를 읽지 못했다(비었거나 주인 교체 중) — 유휴로 물러난다");
                was_unreadable = true;
            }
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        };
        was_unreadable = false;
        // ★ 결함 ⑫의 교훈(Windows 08-27): **내용 없는 스냅숏은 미처리** —
        //   비우고→채우는 틈을 읽었을 수 있다. 기준선을 건드리지 않고 다시 읽는다.
        //   표식(concealed)이 선 것은 이름만이어도 처리 대상이다 — 게이트가 버린다.
        //
        //   ⚠️ 주기는 위와 같은 이유로 **늘린다** — 활동 중 한 틱이 비어도
        //   `IDLE_AFTER_TICKS`(10틱)까지는 활동 주기 그대로라 놓치지 않는다.
        if snap.reps.is_empty() && !snap.concealed {
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        }
        let fp = fingerprint(&snap);
        if last == Some(fp) {
            idle_ticks = next_idle_ticks(idle_ticks, false);
            continue;
        }
        // ★ 부분 스냅숏을 내보내지 않는다 — 자리 잡은 뒤에 넘긴다(아래 [`settle`]).
        let (snap, fp) = settle(backend, snap, fp);
        last = Some(fp);
        idle_ticks = next_idle_ticks(idle_ticks, true);
        diag(&format!(
            "변화 감지 — 표현 {}개 · 표식 {}",
            snap.reps.len(),
            snap.concealed
        ));
        sink(snap);
    }
}

/// 변화를 본 직후 **자리 잡을 때까지** 짧게 다시 읽는다.
///
/// ⚠️ **08-29 Linux 실기가 잡은 결함** — `text/html` 복사가 `text/plain` 하나만 보이는
/// 순간에 잡혀 **같은 복사가 두 항목**이 됐다. 게다가 먼저 것은 표현이 모자라
/// `Text`로 **오분류**됐다. 여러 표현을 올리는 앱이면 일반적으로 이렇게 갈라진다.
///
/// 수신 루프의 디바운스(D-80 · `COALESCE_MS` 500ms)가 원래 이걸 합치는 자리인데,
/// **Linux 폴링 주기가 500ms라 완본이 창 밖에 떨어진다**. 그래서 백엔드가 먼저 다잡는다.
///
/// 규칙은 하나다 — **두 번 같게 읽힐 때까지** 기다리고, 늦게 읽힌 것이 정본이다.
///
/// ⚠️ [`nclip_core::capture::coalesces`]를 쓰지 않는다. 그건 표현이 **늘어나는**
/// 방향(부분 ⊆ 완본)만 합치는데, 실기에서는 **줄어드는** 전이도 나왔다 — 앞 복사의
/// 표현이 잠깐 남아 `{text/plain, gnome-copied-files}` → `{gnome-copied-files}` 로
/// 갔다(08-29 케이스 7). 전이 방향을 가리면 절반만 막힌다.
///
/// ★ **480ms 안의 두 번째 복사를 잃지 않느냐** — 잃지 않는다. 폴링 주기가 이미
/// 500ms라 그보다 짧게 스쳐 간 복사는 **애초에 보이지 않는다**. 자리 잡기는
/// 폴링이 줄 수 있는 것을 깎지 않는다.
///
/// Windows는 재시도(`watch_win` `RETRY_MAX`)로, macOS는 `changeCount`가 원자적이라
/// 겪지 않는다. **변화가 있을 때만 돌므로 유휴 비용은 0이다**(DR-9).
fn settle(backend: Backend, snap: ClipSnapshot, fp: u64) -> (ClipSnapshot, u64) {
    let mut cur = (snap, fp);
    for _ in 0..SETTLE_MAX {
        std::thread::sleep(std::time::Duration::from_millis(SETTLE_MS));
        // ★ 파일 목록은 **다시 요청하지 않는다**(10-05 · T-70) — 지연 제공 소유자(가상 머신 다리 ·
        //   원격 데스크톱)는 요청마다 전송을 시작하거나 다른 임시 경로를 돌려줘, 내용을 견주면 끝내
        //   자리 잡지 못한다. 표현 구성(이름)이 그대로면 자리 잡은 것으로 본다 — 08-29에 본 전이
        //   (`{text/plain, gnome-copied-files}` → `{gnome-copied-files}`)는 이름으로 드러난다.
        if cur
            .0
            .reps
            .iter()
            .any(|r| is_file_listing(&r.format, &r.data))
        {
            let Some(targets) = list_targets(backend) else {
                break;
            };
            let mut have: Vec<String> = cur.0.reps.iter().map(|r| r.format.clone()).collect();
            have.sort();
            if rep_names(&targets) == have {
                return cur;
            }
        }
        let Some(next) = read_snapshot_with(backend) else {
            break;
        };
        if next.reps.is_empty() && !next.concealed {
            break;
        }
        let nfp = fingerprint(&next);
        if nfp == cur.1 {
            // 두 번 같게 읽혔다 — 자리 잡았다.
            return cur;
        }
        diag("아직 자리 잡는 중 — 다시 읽는다");
        cur = (next, nfp);
    }
    cur
}

/// 이번 틱 뒤의 유휴 카운터 — **변화가 있었을 때만 0으로 돌아간다**(순수 · 테스트 대상).
///
/// ★ 못 읽음 · 빈 스냅숏 · 같은 지문은 전부 "변화 없음"이다 — 셋 다 물러나야 한다.
/// ⚠️ 빈 클립보드에서 0으로 되돌리던 것이 08-29 결함이었다(영구 활동 주기 · DR-9).
fn next_idle_ticks(idle: u32, changed: bool) -> u32 {
    if changed {
        0
    } else {
        idle.saturating_add(1)
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

    /// ★ 가상 머신 다리의 임시 경로를 알아본다(10-05 · T-70) — 파일 목록일 때만 버릴 근거가 된다.
    #[test]
    fn vm_staging_paths_are_recognized_only_in_file_listings() {
        let fuse = b"copy\nfile:///var/run/vmblock-fuse/blockdir/AbC123/key.zip";
        let legacy = b"file:///proc/fs/vmblock/mountPoint/AbC123/a.txt\r\n";
        assert!(is_vm_staging(fuse) && is_vm_staging(legacy));
        assert!(!is_vm_staging(b"file:///home/u/a.txt\r\n"));

        assert!(is_file_listing("x-special/gnome-copied-files", fuse));
        assert!(is_file_listing("text/uri-list", legacy));
        // VMware 다리의 변종 — 평문 내용의 첫 줄이 Nautilus 표식.
        let carrier =
            b"x-special/nautilus-clipboard\ncopy\nfile:///var/run/vmblock-fuse/blockdir/x/a";
        assert!(is_file_listing("text/plain", carrier));
        // 그 경로를 **글로** 복사한 것(대화·문서)은 파일 목록이 아니다 — 버리지 않는다.
        let prose = b"/var/run/vmblock-fuse/blockdir/AbC123/key.zip";
        assert!(is_vm_staging(prose) && !is_file_listing("text/plain", prose));
    }

    #[test]
    fn vm_bridge_owner_names() {
        for a in ["vmtoolsd", "Vmware-user", "vmware-user"] {
            assert!(is_vm_bridge_owner(a), "{a}");
        }
        for a in ["nautilus", "code", "vmplayer-like"] {
            assert!(!is_vm_bridge_owner(a), "{a}");
        }
    }

    /// 자리 잡기는 파일 목록을 다시 읽지 않고 **표현 이름 구성**만 견준다.
    #[test]
    fn rep_names_match_snapshot_formats() {
        let t = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(
            rep_names(&t(&[
                "TIMESTAMP",
                "TARGETS",
                "x-special/gnome-copied-files",
                "text/uri-list",
                "UTF8_STRING",
                "text/plain;charset=utf-8",
            ])),
            t(&[
                "text/plain",
                "text/uri-list",
                "x-special/gnome-copied-files"
            ])
        );
        assert_ne!(names_hash(&t(&["a", "b"])), names_hash(&t(&["ab"])));
    }

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

    /// 텍스트 계열은 TARGETS 순서가 아니라 UTF-8 보장 순으로 읽는다 — Mutter 브리지가
    /// `text/plain`(ASCII 이스케이프)을 선두에 두는 실제 순서(09-03 실기)를 그대로 넣는다.
    #[test]
    fn text_targets_read_in_utf8_first_order() {
        let raw: Vec<String> = [
            "TARGETS",
            "TIMESTAMP",
            "MULTIPLE",
            "text/plain",
            "UTF8_STRING",
            "STRING",
            "TEXT",
            "text/plain;charset=utf-8",
            "text/html",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let order: Vec<&str> = read_order(&raw).iter().map(|s| s.as_str()).collect();
        assert_eq!(
            order,
            [
                "text/plain;charset=utf-8",
                "UTF8_STRING",
                "text/plain",
                "TEXT",
                "STRING",
                "TARGETS",
                "TIMESTAMP",
                "MULTIPLE",
                "text/html",
            ]
        );
        assert_eq!(text_rank("text/plain;charset=UTF-8"), Some(0));
        assert_eq!(text_rank("text/html"), None);
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

    /// ★ 08-29 결함 — **빈 클립보드가 영구히 활동 주기로 돌던 것**.
    ///
    /// 못 읽음·빈 스냅숏·같은 지문은 전부 "변화 없음"이라 물러나야 한다. 로그인 직후처럼
    /// 클립보드가 오래 비어 있는 것은 정상 상태인데, 예전 규칙은 그동안 500ms마다
    /// 프로세스를 띄웠다(DR-9 위반).
    #[test]
    fn idle_ticks_back_off_unless_changed() {
        assert_eq!(next_idle_ticks(0, false), 1, "못 읽음/빈 것도 물러난다");
        assert_eq!(next_idle_ticks(9, false), 10);
        assert_eq!(next_idle_ticks(9, true), 0, "변화만 활동으로 되돌린다");
        assert_eq!(next_idle_ticks(u32::MAX, false), u32::MAX, "넘치지 않는다");
        // 변화 없이 굴리면 유휴 상한까지 실제로 올라간다.
        let mut t = 0;
        for _ in 0..IDLE_AFTER_TICKS + 20 {
            t = next_idle_ticks(t, false);
        }
        assert_eq!(interval_ms(t), IDLE_MAX_MS);
    }

    /// ★ 상한은 **읽으면서** 걸린다 — 끝없이 쏟아져도 돌아온다.
    ///
    /// 예전에는 `Command::output()`으로 전량을 담은 **뒤에** 크기를 봤다 —
    /// 이 테스트는 그 구현에서 메모리를 먹으며 끝나지 않는다.
    #[test]
    #[cfg(unix)]
    fn run_bytes_caps_unbounded_output() {
        assert_eq!(
            run_bytes("yes", &[], 4096),
            None,
            "무한 출력은 상한에 걸린다"
        );
        let out = run_bytes("echo", &["hi"], 4096).expect("echo 는 성공한다");
        assert_eq!(out, b"hi\n");
        assert_eq!(run_bytes("nclip-존재하지-않는-명령", &[], 16), None);
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
    /// ★ T-54 — 가벼운 변화 탐지의 대표 타깃: 글이 있으면 가장 믿을 만한 글, 없으면 첫 실타깃.
    #[test]
    fn probe_target_prefers_text_then_first_real_target() {
        let t = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let with_text = t(&["TARGETS", "text/html", "STRING", "UTF8_STRING", "image/png"]);
        assert_eq!(
            probe_target(&with_text).map(String::as_str),
            Some("UTF8_STRING")
        );
        let image = t(&["TARGETS", "TIMESTAMP", "image/png", "image/bmp"]);
        assert_eq!(probe_target(&image).map(String::as_str), Some("image/png"));
        assert_eq!(probe_target(&t(&["TARGETS", "MULTIPLE"])), None);
    }
}
