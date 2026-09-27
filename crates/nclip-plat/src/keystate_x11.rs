//! ★ X 서버 키 상태(`QueryKeymap`) — 팝업 **포커스 잔향 게이트**의 원천(09-27 · T-15d 근인 확정).
//!
//! 왜 필요한가: GNOME(mutter)/XWayland는 팝업이 포커스를 받은 **뒤에** 단축키로 눌려 있던 키를 X 서버 키 상태에
//! 눌림으로 올린다 — winit이 `FocusIn`에서 찍는 합성 Pressed 집합(`query_keymap`)은 그래서 **비어 있다**(09-27
//! 계측: 합성 이벤트 0건). 그 키의 해제는 컴포지터가 삼켜 서버에 **고착**되고 오토리피트가 무한히 온다(첫 리피트는
//! winit에 `repeat: false`). 앱이 스스로 서버 상태를 다시 물어야 잔향을 안다. 한 번 물음 = 왕복 1회(로컬 소켓 ·
//! winit의 FocusIn 처리와 같은 비용). 연결은 프로세스 수명 동안 하나 · 끊기면 다음 호출이 다시 연다.

use std::sync::Mutex;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

static CONN: Mutex<Option<RustConnection>> = Mutex::new(None);

/// 지금 눌려 있다고 X 서버가 믿는 **X keycode**(evdev + 8) 목록. `DISPLAY`가 없거나 연결·요청이 실패하면 `None`
/// (호출자는 게이트 없이 진행 — 정직한 무동작).
#[must_use]
pub fn keys_down() -> Option<Vec<u32>> {
    if !std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
        return None;
    }
    let Ok(mut g) = CONN.lock() else {
        return None;
    };
    if g.is_none() {
        *g = RustConnection::connect(None).ok().map(|(c, _)| c);
    }
    let conn = g.as_ref()?;
    let reply = conn.query_keymap().ok().and_then(|c| c.reply().ok());
    match reply {
        Some(r) => Some(
            r.keys
                .iter()
                .enumerate()
                .flat_map(|(i, b)| {
                    (0..8)
                        .filter(move |bit| b & (1 << bit) != 0)
                        .map(move |bit| (i * 8 + bit) as u32)
                })
                .filter(|kc| *kc >= 8)
                .collect(),
        ),
        None => {
            // 연결이 죽었다(X 서버 재시작 등) — 버리고 다음 호출이 다시 연다.
            *g = None;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    /// 실 X 서버가 있을 때만 — 아무 키도 안 누른 상태면 빈 목록(또는 연결 불가 = None).
    #[test]
    fn keys_down_is_answerable_or_none() {
        let r = super::keys_down();
        if let Some(v) = &r {
            assert!(v.iter().all(|k| *k >= 8 && *k < 256));
        }
    }
}
