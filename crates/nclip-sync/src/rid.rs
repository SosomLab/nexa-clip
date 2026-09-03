//! ★ 랑데부 ID 파생 — **핸들 + 패스프레이즈 → 만남 지점**(docs/09 §6-2 권장안).
//!
//! ```text
//! RID_epoch = PBKDF2-HMAC-SHA256( 패스프레이즈,
//!                                 salt = "nclip-rid-v1" ‖ 핸들 ‖ epoch_day,
//!                                 iters )[..16]
//! ```
//!
//! - 패스프레이즈를 모르면 계산 불가 — 열거 스캔 소멸(R-a).
//! - 핸들이 salt에 들어가 같은 핸들·다른 암호는 서로 못 만난다(R-c).
//! - epoch(UTC 일)마다 회전 — 서버는 어제와 오늘을 잇지 못한다.
//! - ★ 도메인 `"nclip-rid-v1"` = 앱 격리(beep `"nbeep-rid-v1"`과 분리 —
//!   [docs/22 I-1](../../docs/22-upstream-beep-liaison.md) 🟡 공유 규약: 임의 변경 금지).

use sha2::{Digest, Sha256};

/// 랑데부 ID — 서버 `rids` 맵의 키(무의미한 16바이트).
pub type Rid = [u8; 16];

/// 앱 격리 도메인 — ⚠️ 바꾸면 같은 사용자 기기끼리도 못 만난다(docs/22 I-1).
const DOMAIN: &[u8] = b"nclip-rid-v1";

/// PBKDF2 반복 수 — 추측 1회 비용(스캔을 비싸게). 기기 1회 계산엔 수십 ms.
const ITERS: u32 = 60_000;

/// UTC 기준 epoch 일 번호.
#[must_use]
pub fn current_epoch_day() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

/// HMAC-SHA256 — `hmac` crate 없이 sha2로 직접(RFC 2104 · DR-8 의존 0 지향).
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let inner = {
        let mut h = Sha256::new();
        h.update(ipad);
        h.update(msg);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(opad);
    h.update(inner);
    h.finalize().into()
}

/// PBKDF2-HMAC-SHA256 첫 블록(32B) — RID는 16B만 쓰므로 1블록이면 충분.
fn pbkdf2_block1(pass: &[u8], salt: &[u8], iters: u32) -> [u8; 32] {
    // U1 = HMAC(pass, salt ‖ INT(1))
    let mut msg = salt.to_vec();
    msg.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(pass, &msg);
    let mut out = u;
    for _ in 1..iters {
        u = hmac_sha256(pass, &u);
        for (o, b) in out.iter_mut().zip(u.iter()) {
            *o ^= b;
        }
    }
    out
}

/// 핸들+패스프레이즈로 해당 epoch의 RID를 파생한다.
#[must_use]
pub fn derive_rid(handle: &str, passphrase: &str, epoch_day: u64) -> Rid {
    let mut salt = Vec::with_capacity(DOMAIN.len() + handle.len() + 8);
    salt.extend_from_slice(DOMAIN);
    salt.extend_from_slice(handle.as_bytes());
    salt.extend_from_slice(&epoch_day.to_be_bytes());
    let block = pbkdf2_block1(passphrase.as_bytes(), &salt, ITERS);
    let mut rid = [0u8; 16];
    rid.copy_from_slice(&block[..16]);
    rid
}

/// 어제·오늘·내일 3개 — 시계 오차 흡수(docs/07 §3-3의 beep 관례 승계).
#[must_use]
pub fn rids_around(handle: &str, passphrase: &str) -> [Rid; 3] {
    let d = current_epoch_day();
    [
        derive_rid(handle, passphrase, d.saturating_sub(1)),
        derive_rid(handle, passphrase, d),
        derive_rid(handle, passphrase, d + 1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 같은 입력 = 같은 RID(결정적) · 요소 하나만 달라도 다른 RID.
    #[test]
    fn deterministic_and_separated() {
        let a = derive_rid("kiros33", "pw", 20_000);
        assert_eq!(a, derive_rid("kiros33", "pw", 20_000));
        assert_ne!(a, derive_rid("kiros33", "pw", 20_001), "에폭 회전");
        assert_ne!(a, derive_rid("kiros33", "pw2", 20_000), "다른 암호");
        assert_ne!(a, derive_rid("other", "pw", 20_000), "핸들 = salt(R-c)");
    }

    /// HMAC-SHA256 RFC 4231 벡터 #1 — 직접 구현의 정합.
    #[test]
    fn hmac_rfc4231_case1() {
        let key = [0x0bu8; 20];
        let out = hmac_sha256(&key, b"Hi There");
        assert_eq!(
            out[..8],
            [0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53],
            "{out:02x?}"
        );
    }

    /// PBKDF2 RFC 6070 벡터(iters=2) — 반복 접기의 정합.
    #[test]
    fn pbkdf2_rfc6070_iter2() {
        let out = pbkdf2_block1(b"password", b"salt", 2);
        assert_eq!(
            out[..8],
            [0xae, 0x4d, 0x0c, 0x95, 0xaf, 0x6b, 0x46, 0xd3],
            "{out:02x?}"
        );
    }

    /// 어제·오늘·내일 3개 — 전부 다르다.
    #[test]
    fn around_gives_three_distinct() {
        let r = rids_around("h", "p");
        assert_ne!(r[0], r[1]);
        assert_ne!(r[1], r[2]);
    }
}
