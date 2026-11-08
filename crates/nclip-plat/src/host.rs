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

/// ★ 지금 시스템 UTC 오프셋(초 · 동쪽 양수 · 11-08 S9 로그 시각 표시용) — 외부 crate 0.
/// Unix = `localtime_r`의 `tm_gmtoff` · Windows = `GetTimeZoneInformation`(Bias + 표준/DST Bias). 실패 = 0(UTC).
#[must_use]
pub fn utc_offset_secs() -> i32 {
    imp::utc_offset_secs()
}

#[cfg(windows)]
mod imp {
    /// Windows — `COMPUTERNAME` 환경 변수(모든 프로세스에 주입되는 NetBIOS 이름).
    pub(super) fn hostname() -> Option<String> {
        std::env::var("COMPUTERNAME").ok()
    }

    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    }

    /// Bias는 "UTC = 로컬 + Bias"(분 · 서쪽 양수) → 오프셋 = −(Bias + 지금 적용 Bias).
    pub(super) fn utc_offset_secs() -> i32 {
        // SAFETY: 출력 구조체를 넘기는 단순 조회 · 레이아웃은 Win32 TIME_ZONE_INFORMATION 그대로(172B).
        let mut tz: TimeZoneInformation = unsafe { std::mem::zeroed() };
        let id = unsafe { GetTimeZoneInformation(&mut tz) };
        let extra = match id {
            1 => tz.standard_bias,
            2 => tz.daylight_bias,
            _ => 0,
        };
        -(tz.bias + extra) * 60
    }
}

#[cfg(not(windows))]
mod imp {
    /// `struct tm`(glibc·macOS 공통 레이아웃 — 9 × c_int 뒤 `tm_gmtoff: c_long` · `tm_zone`).
    #[repr(C)]
    struct Tm {
        tm_sec: std::os::raw::c_int,
        tm_min: std::os::raw::c_int,
        tm_hour: std::os::raw::c_int,
        tm_mday: std::os::raw::c_int,
        tm_mon: std::os::raw::c_int,
        tm_year: std::os::raw::c_int,
        tm_wday: std::os::raw::c_int,
        tm_yday: std::os::raw::c_int,
        tm_isdst: std::os::raw::c_int,
        tm_gmtoff: std::os::raw::c_long,
        tm_zone: *const std::os::raw::c_char,
    }
    extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub(super) fn utc_offset_secs() -> i32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        // SAFETY: 입력 시각과 출력 구조체만 넘기는 재진입 안전 호출 · 실패 = null → 0.
        let mut tm: Tm = unsafe { std::mem::zeroed() };
        let r = unsafe { localtime_r(&now, &mut tm) };
        if r.is_null() {
            0
        } else {
            i32::try_from(tm.tm_gmtoff).unwrap_or(0)
        }
    }
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
