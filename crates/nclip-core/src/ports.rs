//! 포트 — **core가 선언하고 어댑터가 구현한다**(의존성 역전).
//!
//! `nclip-plat`이 OS별로 구현하고 본체(`nexa-clip`)가 조립 시점에 주입한다.
//! core는 Win32도 AppKit도 Wayland도 모른다.

/// 읽어 온 표현 하나 — **이름 + 날바이트**.
///
/// ⚠️ [`Representation`](crate::item::Representation)이 **아니다.**
/// 그쪽은 `blob_id`(암호문 해시)를 들고 있는데, 감시 계층은 **키를 모른다** —
/// 암호화는 저장 계층의 일이다. ★ **모르는 것을 채우게 만들지 않는다.**
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RawRep {
    /// OS가 준 포맷 이름 그대로(예: `CF_HTML` · `Art::GVML ClipFormat`).
    pub format: String,
    /// 날바이트. **비어 있을 수 있다** — 핸들 기반 포맷(`CF_BITMAP`·`CF_ENHMETAFILE`)은
    /// 메모리 블록이 아니라서 이름만 담는다([`ClipSnapshot::reps`] 주의).
    pub data: Vec<u8>,
}

/// 한 번의 클립보드 변화에서 읽어낸 **표현 묶음**.
///
/// ★ **해석은 하지 않고 이름째** 담는다([docs/12 F-1](../../../docs/12-clipboard-formats.md)).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClipSnapshot {
    /// 보유 표현 전부(이름 + 날바이트).
    pub reps: Vec<RawRep>,
    /// 출처 앱 표시 이름(알아낼 수 있으면).
    pub source_app: Option<String>,
    /// ★ **민감 표식이 붙어 있었다** — 비밀번호 관리자 등이 *"기록하지 마"* 를 보낸 경우.
    /// 이게 `true`면 **저장하지 않는다**(FR-S-1 · fail-closed).
    pub concealed: bool,
    /// OS가 주는 변경 일련번호 — ★ **같은 변화를 두 번 받는 것을 막는다**.
    /// 모르는 환경에서는 0.
    pub seq: u64,
}

impl ClipSnapshot {
    /// 포맷 이름만 뽑는다([`crate::capture::classify`]에 그대로 넘긴다).
    #[must_use]
    pub fn formats(&self) -> Vec<&str> {
        self.reps.iter().map(|r| r.format.as_str()).collect()
    }

    /// 파일 항목의 **경로 목록**(파일 표현이 없으면 빈 목록).
    ///
    /// `CF_HDROP` → `text/uri-list` 순으로 본다. macOS `public.file-url`은 한 항목당
    /// 표현이 하나씩 오므로 **모아서** 돌려준다.
    ///
    /// ★ Linux 파일 관리자 표현(`x-special/…-copied-files`)은 **첫 줄이 `cut`/`copy`** 다 —
    /// [`crate::capture::parse_uri_list`]가 `file://` 줄만 받으므로 표식 줄은 저절로 걸러진다.
    ///
    /// ⚠️ **같은 경로를 두 번 담지 않는다** — GNOME은 `text/uri-list`와
    /// `x-special/gnome-copied-files`를 **함께** 내놓아서, 그대로 두면 파일 하나를
    /// 복사해도 목록에 둘로 보인다(08-29).
    #[must_use]
    pub fn file_paths(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for r in &self.reps {
            let found = match r.format.as_str() {
                "CF_HDROP" => crate::capture::parse_hdrop(&r.data),
                "text/uri-list"
                | "public.file-url"
                | "x-special/gnome-copied-files"
                | "x-special/KDE-copied-files"
                | "x-special/nautilus-clipboard" => std::str::from_utf8(&r.data)
                    .map(crate::capture::parse_uri_list)
                    .unwrap_or_default(),
                _ => continue,
            };
            for p in found {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        out
    }

    /// 파일 항목의 **이름만**(목록 표시용 — 전체 경로는 길고 사생활이다).
    ///
    /// ★ macOS NFD 자모는 음절로 조합해 준다(08-29 실기 — 자모가 낱낱이 보였다).
    /// 경로([`Self::file_paths`])는 **원본 그대로**다 — 정규화는 표시에만.
    #[must_use]
    pub fn file_names(&self) -> Vec<String> {
        self.file_paths()
            .iter()
            .map(|p| crate::capture::compose_hangul_nfd(crate::capture::base_name(p)))
            .collect()
    }

    /// 복사가 일어난 **출처 페이지 URL**(Chromium 계열이 실어 보낸다). 없으면 `None`.
    ///
    /// ★ D-79의 재료 — 브라우저 암호 관리자 복사는 민감 표식이 없어서(08-27 실기)
    /// 이 URL로만 알아볼 수 있다([`crate::capture::is_password_manager_url`]).
    #[must_use]
    pub fn source_url(&self) -> Option<String> {
        let r = self
            .reps
            .iter()
            .find(|r| r.format == "Chromium internal source URL")?;
        let text = String::from_utf8_lossy(&r.data);
        let trimmed = text.trim_end_matches('\0').trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// 평문 표현의 내용.
    ///
    /// ★ **가장 정확히 풀 수 있는 표현부터** 본다([`plain_rank`](crate::capture::plain_rank)) —
    /// `CF_UNICODETEXT`(UTF-16LE)가 코드 페이지에 묶인 `CF_TEXT`보다 먼저다.
    /// ⚠️ 순서를 뒤집으면 한글이 깨진다.
    #[must_use]
    pub fn plain_text(&self) -> Option<String> {
        let mut best: Option<(u8, &RawRep)> = None;
        for r in &self.reps {
            if let Some(rank) = crate::capture::plain_rank(&r.format) {
                if best.is_none_or(|(b, _)| rank < b) {
                    best = Some((rank, r));
                }
            }
        }
        let (_, r) = best?;
        crate::capture::decode_plain(&r.format, &r.data)
    }
}

/// 감시 계층이 이 환경에서 **무엇을 할 수 있는가**.
///
/// ⚠️ 미지원을 조용한 빈 목록으로 숨기지 않기 위해 존재한다 —
/// Wayland/GNOME처럼 **구조적으로 불가능한 환경**을 사용자에게 정직하게 알린다
/// ([docs/02 R-4](../../../docs/02-roadmap.md)).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WatchCapability {
    /// 정상 감시 가능.
    Supported {
        /// 구현 이름(진단 표시용 — 예: `win32-listener` · `mac-pollling` · `x11-xfixes`).
        backend: &'static str,
    },
    /// 이 환경에서는 수집할 수 없다.
    Unsupported {
        /// 사용자에게 보여줄 사유(진단·안내용).
        reason: UnsupportedReason,
    },
}

/// 감시가 불가능한 이유.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnsupportedReason {
    /// Wayland 컴포지터가 data-control 프로토콜을 제공하지 않는다(GNOME 등).
    WaylandNoDataControl,
    /// 표시 서버에 연결할 수 없다(헤드리스 등).
    NoDisplayServer,
    /// 필요한 OS 도구가 없다(Linux — `wl-clipboard`/`xclip` 이 설치 대상).
    MissingTool(&'static str),
    /// 이 타깃은 아직 구현되지 않았다.
    NotImplemented,
}

/// 감시 시작 실패.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WatchError {
    /// 이 환경에서는 불가능하다(사용자 안내 대상).
    Unsupported(UnsupportedReason),
    /// OS 호출이 실패했다(진단 문자열).
    Os(String),
}

/// ★ **클립보드 감시 포트**(FR-C-1).
///
/// 구현은 OS마다 모델이 다르다 — Windows는 이벤트, macOS는 폴링, X11은 셀렉션 알림,
/// Wayland는 컴포지터 프로토콜([docs/20 §3-1](../../../docs/20-implementation-spec.md)).
/// **그 차이는 전부 이 트레이트 뒤에 있다.**
pub trait ClipboardWatch: core::fmt::Debug {
    /// 이 환경에서 무엇이 가능한지. **`start` 전에** 물어볼 수 있어야 한다(온보딩 점검).
    fn capability(&self) -> WatchCapability;

    /// 감시를 시작한다. 변화마다 `on_change`가 불린다.
    ///
    /// # Errors
    /// 환경이 지원하지 않거나 OS 호출이 실패하면 [`WatchError`].
    fn start(&mut self, on_change: Box<dyn Fn(ClipSnapshot) + Send>) -> Result<(), WatchError>;

    /// 일시 정지/재개(FR-C-11). 정지 중에는 `on_change`가 불리지 않는다.
    fn set_paused(&mut self, paused: bool);

    /// ★ **다음 1건만 무시**(FR-C-13 · Maccy 선례) —
    /// 토글과 달리 **다시 켜는 것을 잊을 수 없다**([docs/14 §2-2](../../../docs/14-settings-registry.md)).
    fn skip_next(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 미지원 환경도 **타입으로 표현된다** — 조용히 빈 목록이 되지 않는다.
    #[test]
    fn unsupported_carries_reason() {
        let cap = WatchCapability::Unsupported {
            reason: UnsupportedReason::WaylandNoDataControl,
        };
        match cap {
            WatchCapability::Unsupported { reason } => {
                assert_eq!(reason, UnsupportedReason::WaylandNoDataControl);
            }
            WatchCapability::Supported { .. } => panic!("지원으로 잘못 읽혔다"),
        }
    }

    /// ★ D-79 — 출처 URL이 표현에서 나온다(널 종단·공백 정리 포함).
    #[test]
    fn source_url_comes_from_chromium_rep() {
        let s = ClipSnapshot {
            reps: vec![
                RawRep {
                    format: "CF_UNICODETEXT".into(),
                    data: vec![0x70, 0x00],
                },
                RawRep {
                    format: "Chromium internal source URL".into(),
                    data: b"edge://wallet/passwordsDetail\0".to_vec(),
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            s.source_url().as_deref(),
            Some("edge://wallet/passwordsDetail")
        );
        assert!(ClipSnapshot::default().source_url().is_none());
        // 빈 내용은 None — 있는 척하지 않는다.
        let empty = ClipSnapshot {
            reps: vec![RawRep {
                format: "Chromium internal source URL".into(),
                data: b"\0".to_vec(),
            }],
            ..Default::default()
        };
        assert!(empty.source_url().is_none());
    }

    /// 민감 표식이 붙은 스냅숏은 **기본이 저장 금지**임을 모델이 드러낸다.
    #[test]
    fn concealed_defaults_false_but_is_explicit() {
        let s = ClipSnapshot::default();
        assert!(!s.concealed);
        let c = ClipSnapshot {
            concealed: true,
            ..Default::default()
        };
        assert!(c.concealed);
    }
}

#[cfg(test)]
mod linux_file_manager_tests {
    use crate::{ClipSnapshot, RawRep};

    fn snap(reps: &[(&str, &[u8])]) -> ClipSnapshot {
        ClipSnapshot {
            reps: reps
                .iter()
                .map(|(f, d)| RawRep {
                    format: (*f).to_string(),
                    data: d.to_vec(),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// ★ 08-29 — GNOME/Nautilus **잘라내기**는 `text/uri-list` 없이 이것만 올 수 있다.
    /// 첫 줄 `cut`은 표식이라 경로가 아니다.
    #[test]
    fn gnome_copied_files_yields_paths_without_marker_line() {
        let s = snap(&[(
            "x-special/gnome-copied-files",
            b"cut\nfile:///home/u/%ED%95%9C%EA%B8%80.txt\nfile:///home/u/b.txt",
        )]);
        assert_eq!(
            s.file_paths(),
            vec!["/home/u/한글.txt".to_string(), "/home/u/b.txt".to_string()]
        );
        assert_eq!(
            s.file_names(),
            vec!["한글.txt".to_string(), "b.txt".to_string()]
        );
    }

    /// `copy` 표식도 같은 자리다 — 잘라내기만 되는 게 아니다.
    #[test]
    fn gnome_copy_marker_is_also_skipped() {
        let s = snap(&[("x-special/gnome-copied-files", b"copy\nfile:///tmp/a.txt")]);
        assert_eq!(s.file_paths(), vec!["/tmp/a.txt".to_string()]);
    }

    /// ★ GNOME은 두 표현을 **함께** 내놓는다 — 그대로 두면 파일 하나가 둘로 보인다.
    #[test]
    fn duplicate_paths_across_representations_collapse() {
        let s = snap(&[
            ("text/uri-list", b"file:///tmp/a.txt\r\n"),
            ("x-special/gnome-copied-files", b"cut\nfile:///tmp/a.txt"),
        ]);
        assert_eq!(s.file_paths(), vec!["/tmp/a.txt".to_string()], "한 번만");
    }

    /// KDE(Dolphin) 이름도 같은 취급 — 잘라내기 표식은 **곁다리**로 따로 걸러진다.
    #[test]
    fn kde_copied_files_and_cutselection() {
        let s = snap(&[
            ("x-special/KDE-copied-files", b"cut\nfile:///tmp/k.txt"),
            ("application/x-kde-cutselection", b"1"),
        ]);
        assert_eq!(s.file_paths(), vec!["/tmp/k.txt".to_string()]);
        assert!(crate::capture::is_metadata_format(
            "application/x-kde-cutselection"
        ));
        assert!(crate::capture::is_files_format(
            "x-special/KDE-copied-files"
        ));
    }
}
