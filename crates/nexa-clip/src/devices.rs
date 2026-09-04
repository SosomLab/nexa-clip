//! ★ 기기 레지스트리(09-03 — "Display name으로 같은 UserId의 기기를 구별"):
//! 종단 세션으로 **인증된 PeerId**와 상대가 보낸 표시 이름을 기억하고, 연결 여부를 표시한다.
//!
//! 영속 = `data/devices.txt`(자체 직렬화 · DR-37): 한 줄 = `v1 <peer_hex> <first> <last> <os> <name…>`.
//! 이름은 마지막 필드라 공백을 품을 수 있다. 미지 줄은 버린다(전방 호환).
//!
//! ⚠️ 여기 있다고 **신뢰된** 기기는 아니다(docs/09 §6-3 — 승인은 DeviceList 단계). 지금 단계는
//! "만났고 이름을 안다"까지이며, 클립보드 전파는 승인 전까지 열지 않는다.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 알려진 기기 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceEntry {
    /// PeerId 16진(64자).
    pub hex: String,
    /// 상대가 보낸 표시 이름(무해화됨).
    pub name: String,
    /// OS 태그.
    pub os: String,
    /// 처음 만난 시각(unix 초).
    pub first_seen: u64,
    /// 마지막으로 살아 있던 시각(unix 초).
    pub last_seen: u64,
    /// 지금 종단 세션이 살아 있는가.
    pub online: bool,
    /// ★ 사용자가 승인한 기기(09-04 — docs/09 §6-3: 승인 전엔 클립보드를 주고받지 않는다).
    pub approved: bool,
    /// 지금 세션의 경로(`LAN`/`relay`) — 표시용 · 비영속.
    pub via: String,
}

static DEVICES: Mutex<Vec<DeviceEntry>> = Mutex::new(Vec::new());

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 목록 스냅숏(표시용 — 온라인 먼저, 그다음 최근 순).
pub(crate) fn list() -> Vec<DeviceEntry> {
    let mut v = DEVICES.lock().map(|g| g.clone()).unwrap_or_default();
    v.sort_by(|a, b| b.online.cmp(&a.online).then(b.last_seen.cmp(&a.last_seen)));
    v
}

/// 알려진 기기의 PeerId 16진 목록(다이얼 대상).
pub(crate) fn known_hex() -> Vec<String> {
    DEVICES
        .lock()
        .map(|g| g.iter().map(|d| d.hex.clone()).collect())
        .unwrap_or_default()
}

/// 지금 온라인인가.
pub(crate) fn is_online(hex: &str) -> bool {
    DEVICES
        .lock()
        .map(|g| g.iter().any(|d| d.hex == hex && d.online))
        .unwrap_or(false)
}

/// 세션 성립 + 인사 수신 — 있으면 갱신, 없으면 추가. 반환 = 새 기기였는가.
pub(crate) fn upsert_online(hex: &str, name: &str, os: &str, via: &str) -> bool {
    let now = now_secs();
    let Ok(mut g) = DEVICES.lock() else {
        return false;
    };
    if let Some(d) = g.iter_mut().find(|d| d.hex == hex) {
        d.name = name.to_string();
        d.os = os.to_string();
        d.last_seen = now;
        d.online = true;
        d.via = via.to_string();
        false
    } else {
        g.push(DeviceEntry {
            hex: hex.to_string(),
            name: name.to_string(),
            os: os.to_string(),
            first_seen: now,
            last_seen: now,
            online: true,
            approved: false,
            via: via.to_string(),
        });
        true
    }
}

/// 승인됐는가(전파 게이트 — 보낼 때·받을 때 모두).
pub(crate) fn is_approved(hex: &str) -> bool {
    DEVICES
        .lock()
        .map(|g| g.iter().any(|d| d.hex == hex && d.approved))
        .unwrap_or(false)
}

/// ★ 지금 연결된 기기를 전부 승인(설정 버튼) — 반환 = 새로 승인된 수.
pub(crate) fn approve_online() -> usize {
    let Ok(mut g) = DEVICES.lock() else {
        return 0;
    };
    let mut n = 0;
    for d in g.iter_mut().filter(|d| d.online && !d.approved) {
        d.approved = true;
        n += 1;
    }
    n
}

/// 세션 종료.
pub(crate) fn set_offline(hex: &str) {
    if let Ok(mut g) = DEVICES.lock() {
        if let Some(d) = g.iter_mut().find(|d| d.hex == hex) {
            d.online = false;
            d.last_seen = now_secs();
        }
    }
}

/// 전부 오프라인(테스트용 — 실경로는 `all_offline_via`).
#[cfg(test)]
pub(crate) fn all_offline() {
    if let Ok(mut g) = DEVICES.lock() {
        let now = now_secs();
        for d in g.iter_mut().filter(|d| d.online) {
            d.online = false;
            d.last_seen = now;
        }
    }
}

/// ★ 한 경로만 오프라인(09-04) — 릴레이가 끊겨도 LAN 세션은 산다(그 역도).
pub(crate) fn all_offline_via(via: &str) {
    if let Ok(mut g) = DEVICES.lock() {
        let now = now_secs();
        for d in g.iter_mut().filter(|d| d.online && d.via == via) {
            d.online = false;
            d.last_seen = now;
        }
    }
}

/// 파일에서 복원(부팅 1회) — 전부 오프라인으로 시작.
pub(crate) fn load(path: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let mut v = Vec::new();
    for line in text.lines() {
        // v2 = 승인 필드 추가(09-04) · v1(09-03)은 미승인으로 읽는다.
        let ver = line.split(' ').next().unwrap_or("");
        let (nfields, approved_field) = match ver {
            "v1" => (6, false),
            "v2" => (7, true),
            _ => continue,
        };
        let mut it = line.splitn(nfields, ' ');
        it.next();
        let (Some(hex), Some(first), Some(last), Some(os)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        let approved = approved_field && it.next() == Some("A");
        let name = it.next().unwrap_or("").to_string();
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        v.push(DeviceEntry {
            hex: hex.to_string(),
            name,
            os: os.to_string(),
            first_seen: first.parse().unwrap_or(0),
            last_seen: last.parse().unwrap_or(0),
            online: false,
            approved,
            via: String::new(),
        });
    }
    if let Ok(mut g) = DEVICES.lock() {
        *g = v;
    }
}

/// 파일로 저장(temp 쓰기 후 rename — pinfile과 같은 원자성).
pub(crate) fn save(path: &std::path::Path) -> std::io::Result<()> {
    let lines: Vec<String> = DEVICES
        .lock()
        .map(|g| {
            g.iter()
                .map(|d| {
                    format!(
                        "v2 {} {} {} {} {} {}",
                        d.hex,
                        d.first_seen,
                        d.last_seen,
                        if d.os.is_empty() { "-" } else { &d.os },
                        if d.approved { "A" } else { "-" },
                        d.name
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, lines.join("\n") + "\n")?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_file_keeps_name_with_spaces() {
        let dir = std::env::temp_dir().join(format!("nclip-devices-{}", std::process::id()));
        let path = dir.join("devices.txt");
        let hex = "ab".repeat(32);
        assert!(upsert_online(&hex, "작업용 PC 2", "windows", "relay"));
        assert!(
            !upsert_online(&hex, "작업용 PC 2", "windows", "LAN"),
            "두 번째는 갱신"
        );
        assert_eq!(list()[0].via, "LAN");
        save(&path).expect("저장");
        all_offline();
        load(&path);
        let l = list();
        let d = l.iter().find(|d| d.hex == hex).expect("복원된 기기");
        assert_eq!(d.name, "작업용 PC 2");
        assert_eq!(d.os, "windows");
        assert!(!d.online);
        assert!(!d.approved, "v2 미승인 필드");
        // 승인은 온라인 기기만 · 파일을 왕복해도 남는다.
        assert!(!upsert_online(&hex, "작업용 PC 2", "windows", "relay"));
        assert_eq!(approve_online(), 1);
        save(&path).expect("저장");
        load(&path);
        assert!(is_approved(&hex));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
