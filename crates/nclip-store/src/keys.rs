//! 키 계층(v1) — ★ **마스터 키는 무작위 생성 → 래핑 한 겹**(beep ADR-0005 · D-21 제약 계승).
//!
//! 기기 키에서 직접 파생하면 보호 수준을 바꿀 때마다 전 기록 재암호화가 필요하다 —
//! 간접 한 겹이면 나중에 **패스프레이즈 래핑(DR-38 후속)** 을 키 파일 교체만으로 더할 수 있다.
//!
//! ```text
//! store/
//! ├─ device.key   32B 무작위(기기 로컬 비밀 — DR-38 "꺼도 기기 키 암호화 유지"의 그 키)
//! └─ keys         마스터 키(32B 무작위)를 device.key로 봉인한 봉투
//! ```
//!
//! ⚠️ **정직한 한계(v1)**: `device.key`가 데이터 폴더에 평문으로 있다 — 세그먼트·blob만
//! 새는 사고(부분 백업·동기화 폴더 유출)는 막지만, **폴더째 복사에는 못 버틴다**.
//! OS 비밀 저장(DPAPI·Keychain·Secret Service) 결합과 패스프레이즈 래핑이 후속이다.
//!
//! `keys`가 열리지 않으면(기기 키 교체·손상) — **fail-closed**: 기존 세그먼트·blob을
//! `.locked`로 보관하고 새로 시작한다(beep `archive_name` 문법 — 보관은 삭제가 아니다).

use std::io;
use std::path::Path;

use crate::sealed;

const DOMAIN_KEYS: &[u8] = b"keys-v1";

fn random32() -> io::Result<[u8; 32]> {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).map_err(|_| io::Error::other("OS 난수 실패"))?;
    Ok(k)
}

/// 파일이 있으면 읽고, 없으면 32B 무작위를 만들어 쓴다(0600 — unix 한정).
fn load_or_create_secret(path: &Path) -> io::Result<[u8; 32]> {
    if let Ok(b) = std::fs::read(path) {
        if b.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&b);
            return Ok(k);
        }
        // 길이가 틀린 파일은 손상 — 덮지 않고 실패한다(호출측이 보관 정책을 정한다).
        return Err(io::Error::other(format!("{} 손상(길이)", path.display())));
    }
    let k = random32()?;
    // ★ 생성 시점부터 0600(09-05) — 쓰고 나서 chmod 하면 그 사이 umask 모드로 잠깐 열린다.
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    {
        use std::io::Write as _;
        opts.open(path)?.write_all(&k)?;
    }
    Ok(k)
}

/// 마스터 키 적재 결과.
pub(crate) enum MasterLoad {
    /// 기존 마스터가 열렸다(또는 첫 실행이라 새로 만들었다).
    Ready([u8; 32]),
    /// ★ `keys`가 있는데 열리지 않았다 — 호출측은 기존 기록을 보관하고 새로 시작해야 한다.
    Mismatch,
}

/// `dir/device.key` + `dir/keys`에서 마스터 키를 적재한다(없으면 생성).
pub(crate) fn load_master(dir: &Path) -> io::Result<MasterLoad> {
    let device = load_or_create_secret(&dir.join("device.key"))?;
    let keys_path = dir.join("keys");
    if let Ok(wrapped) = std::fs::read(&keys_path) {
        return Ok(match sealed::open(DOMAIN_KEYS, &device, &wrapped) {
            Some(m) if m.len() == 32 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&m);
                MasterLoad::Ready(k)
            }
            _ => MasterLoad::Mismatch,
        });
    }
    let master = random32()?;
    std::fs::write(&keys_path, sealed::seal(DOMAIN_KEYS, &device, &master)?)?;
    Ok(MasterLoad::Ready(master))
}

/// 잠긴 자리의 보관 이름 — `<원본>.locked`, 이미 있으면 `-1`·`-2`…(beep 이식 ·
/// 덮어쓰기 금지 — 보관은 삭제가 아니다). 100개 넘으면 None(비정상 — 현행 유지).
pub(crate) fn archive_name(path: &Path) -> Option<std::path::PathBuf> {
    let base = path.as_os_str().to_string_lossy().into_owned();
    for n in 0..100 {
        let cand = if n == 0 {
            std::path::PathBuf::from(format!("{base}.locked"))
        } else {
            std::path::PathBuf::from(format!("{base}.locked-{n}"))
        };
        if !cand.exists() {
            return Some(cand);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nclip-keys-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 같은 폴더에서 두 번 열면 같은 마스터가 온다 — 이것이 영속의 전제다.
    #[test]
    fn master_is_stable_across_opens() {
        let d = tmp("stable");
        let MasterLoad::Ready(a) = load_master(&d).unwrap() else {
            panic!("첫 생성")
        };
        let MasterLoad::Ready(b) = load_master(&d).unwrap() else {
            panic!("재적재")
        };
        assert_eq!(a, b);
        let _ = std::fs::remove_dir_all(d);
    }

    /// 기기 키가 바뀌면 Mismatch — 조용히 새 키로 여는 일은 없다(fail-closed).
    #[test]
    fn changed_device_key_is_mismatch_not_silent_reset() {
        let d = tmp("mismatch");
        assert!(matches!(load_master(&d).unwrap(), MasterLoad::Ready(_)));
        std::fs::write(d.join("device.key"), [9u8; 32]).unwrap();
        assert!(matches!(load_master(&d).unwrap(), MasterLoad::Mismatch));
        let _ = std::fs::remove_dir_all(d);
    }
}
