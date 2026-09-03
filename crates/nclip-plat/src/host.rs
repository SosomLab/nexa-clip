//! 호스트명 취득 — 기기 표시 이름 기본값의 **원료**(09-03 · beep `nbeep-plat/src/host.rs` 이식).
//!
//! ⚠️ 여기서 얻은 문자열은 실명을 품을 수 있다("{실명}의 MacBook" 등). **그대로 쓰지 말 것** —
//! 반드시 `nclip_sync::name::default_display_name`(정제·폴백)을 거친다. 이 모듈은 원시
//! 문자열만 돌려주고 판단하지 않는다(플랫폼 경계).

/// OS 호스트명(원시). 취득 실패 시 `None` — 호출자는 지문 라벨로 폴백한다.
#[must_use]
pub fn hostname() -> Option<String> {
    imp::hostname().filter(|s| !s.trim().is_empty())
}

#[cfg(windows)]
mod imp {
    /// Windows — `COMPUTERNAME` 환경 변수(모든 프로세스에 주입되는 NetBIOS 이름).
    pub(super) fn hostname() -> Option<String> {
        std::env::var("COMPUTERNAME").ok()
    }
}

#[cfg(not(windows))]
mod imp {
    /// Unix — `HOSTNAME` 환경 변수 → `/etc/hostname`(Linux). 외부 crate·FFI 없이(DR-8);
    /// macOS는 둘 다 없을 수 있어 `None` = 지문 라벨 폴백(정직한 한계).
    pub(super) fn hostname() -> Option<String> {
        if let Ok(h) = std::env::var("HOSTNAME") {
            return Some(h);
        }
        std::fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_string())
    }
}
