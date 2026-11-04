//! GAP-042 — winit 0.30.13 `TaskbarCreated` 교착 우회의 **호출 지점**(3-OS · 11-04 nexa-dir3 이식).
//!
//! winit 창을 만드는 **모든** 곳(`el.create_window(...)`)이 바로 뒤에 [`guard_taskbar_created`]를 부른다. Windows는
//! [`nclip_plat::wintaskbar`]가 서브클래스를 달고, 다른 OS는 no-op(`Err`)이다 — 호출부는 조용히 넘긴다.
//! 정적 가드 시험 [`tests::taskbar_guard_follows_every_window_creation`]이 파일별 건수를 대조해 새 창에서 호출을 빠뜨리면 실패한다.
//! **제거 조건**: winit을 상류 수정 #4563이 든 판(0.31 이후)으로 올릴 때 이 모듈 · 호출 지점 · `nclip_plat::wintaskbar`를 함께 지운다.

/// 창을 만든 직후 `TaskbarCreated` 교착 우회 서브클래스를 단다(Windows) — winit 0.30.13이 이 메시지를 창 상태 잠금을 쥔 채
/// 처리하다 테마 변경 재진입으로 영구 대기하는 결함을 앱 쪽에서 막는다(포크 없음 · nexa-dir3 사용자 결정 11-04 ⓑ와 동일).
/// 다른 OS = `Err("unsupported")`.
pub(crate) fn guard_taskbar_created(win: &winit::window::Window) -> Result<(), &'static str> {
    #[cfg(windows)]
    {
        use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
        let Ok(h) = win.window_handle() else {
            return Err("window_handle");
        };
        let RawWindowHandle::Win32(w) = h.as_raw() else {
            return Err("not win32");
        };
        if nclip_plat::wintaskbar::install(w.hwnd.get()) {
            Ok(())
        } else {
            Err("SetWindowSubclass")
        }
    }
    #[cfg(not(windows))]
    {
        let _ = win;
        Err("unsupported")
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// GAP-042 정적 가드(3-OS): winit 창을 만드는 줄(`el.create_window(`)마다 같은 파일에 `guard_taskbar_created(&win)` 호출이
    /// 하나씩 있어야 한다(주석 줄 제외 · 파일별 건수 대조). 새 창을 추가하면 생성 직후 호출도 함께 넣는다 — winit 0.30.13
    /// `TaskbarCreated` 교착을 창마다 막는 유일한 길이라서. winit을 0.31 이후로 올리면 이 시험과 호출을 함께 지운다.
    #[test]
    fn taskbar_guard_follows_every_window_creation() {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).expect("read_dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        fn count(src: &str, needle: &str) -> usize {
            src.lines()
                .filter(|l| {
                    let t = l.trim_start();
                    !t.starts_with("//") && !t.starts_with("///") && t.contains(needle)
                })
                .count()
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&root, &mut files);
        let mut total = 0;
        let mut bad = Vec::new();
        for f in files {
            let rel = f
                .strip_prefix(&root)
                .expect("strip_prefix")
                .to_string_lossy()
                .replace('\\', "/");
            let src = std::fs::read_to_string(&f).expect("read");
            // 찾는 글을 둘로 쪼개 이 줄 자신이 세어지지 않게.
            let created = count(&src, concat!("el.create_", "window("));
            let guarded = count(&src, concat!("guard_taskbar_", "created(&win)"));
            total += created;
            if created != guarded {
                bad.push(format!("{rel}: create_window {created} · guard {guarded}"));
            }
        }
        assert!(bad.is_empty(), "GAP-042 가드 누락/과잉: {bad:?}");
        assert!(
            total >= 4,
            "winit 창 생성 지점이 줄었다({total} · 기대 4 = 메인·팝업·설정·데모) — 표를 확인"
        );
    }
}
