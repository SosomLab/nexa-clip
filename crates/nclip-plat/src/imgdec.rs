//! 이미지 격리 디코드 **어댑터**(T-19) — ★ **본체는 이미지 파서를 링크하지 않는다**.
//!
//! [`nclip-imgdec`](../../nclip-imgdec) 자식 프로세스에 바이트를 보내고, 상한 걸린
//! RGBA만 받는다(beep R-5·FR-S-12 구조 그대로). 자식이 크래시·오염돼도 본체는
//! `None`을 받을 뿐이고, **시간 상한은 부모가 kill로 강제**한다(압축 폭탄의 CPU 소진 차단).
//!
//! 이식 원본: `nexa-beep` `crates/nexa-beep/src/imgdec.rs`
//! (decode/encode-raw 경로만 — 아바타·와이어·격리함은 beep 도메인).

use std::io::{Read as _, Write as _};
use std::process::{Command, Stdio};

/// 디코드 대기 상한 — 초과는 손상·폭탄 취급(kill).
const DECODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// 인코드 대기 상한 — 4K RGBA(33MiB)의 PNG 압축은 3초를 넘을 수 있다.
const ENCODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 워커 실행 파일 경로 — 본체 옆에 동봉된다(없으면 조용히 `None` = 미리보기 폴백).
fn worker_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let p = dir.join(if cfg!(windows) {
        "nclip-imgdec.exe"
    } else {
        "nclip-imgdec"
    });
    p.exists().then_some(p)
}

/// 워커 스폰 공통 준비 — Windows는 **콘솔 창 없이**(CREATE_NO_WINDOW).
/// 본체가 windows 서브시스템이 되면 콘솔 자식이 디코드마다 창을 번쩍인다(beep 08-20 실기).
fn worker_command(path: &std::path::Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        let mut c = Command::new(path);
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    }
    #[cfg(not(windows))]
    {
        Command::new(path)
    }
}

/// 자식 stdout을 시간 상한 안에서 끝까지 읽는다. 초과·실패 = kill 후 `None`.
fn run_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Option<Vec<u8>> {
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let ok = stdout.read_to_end(&mut out).is_ok();
        let _ = tx.send(ok.then_some(out));
    });
    let out = match rx.recv_timeout(timeout) {
        Ok(Some(out)) => out,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    child.wait().ok()?.success().then_some(out)
}

/// `NIMG` 응답 해석 — ★ **본체 측 재검증**(자식이 오염됐어도 여기서 상한이 선다).
fn parse_nimg(out: &[u8], max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    if out.len() < 12 || &out[..4] != b"NIMG" {
        return None;
    }
    let w = u32::from_le_bytes(out[4..8].try_into().ok()?);
    let h = u32::from_le_bytes(out[8..12].try_into().ok()?);
    if w == 0 || h == 0 || w > max_side.max(1) * 2 || h > max_side.max(1) * 2 {
        return None;
    }
    let need = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
    (out.len() == 12 + need).then(|| (w, h, out[12..].to_vec()))
}

/// 격리 디코드(PNG/JPEG → 축소 RGBA) — 실패 사유는 구분하지 않는다(전부 "없음").
#[must_use]
pub fn decode_isolated(bytes: &[u8], max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    let path = worker_path()?;
    let mut child = worker_command(&path)
        .args(["--max-side", &max_side.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // 입력을 쓰고 stdin을 닫아야 자식이 EOF를 본다.
    child.stdin.take()?.write_all(bytes).ok()?;
    parse_nimg(&run_with_timeout(child, DECODE_TIMEOUT)?, max_side)
}

/// 원시 RGBA → **원본 크기 PNG** — 인코딩도 워커 몫(본체는 인코더도 링크하지 않는다).
#[must_use]
pub fn encode_raw_isolated(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let path = worker_path()?;
    let mut child = worker_command(&path)
        .args(["--encode-raw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut si = child.stdin.take()?;
        si.write_all(&w.to_le_bytes()).ok()?;
        si.write_all(&h.to_le_bytes()).ok()?;
        si.write_all(rgba).ok()?;
    }
    let out = run_with_timeout(child, ENCODE_TIMEOUT)?;
    out.starts_with(&[0x89, b'P', b'N', b'G']).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NIMG 해석 — 본체 측 재검증(크기 위조·길이 불일치 거부).
    #[test]
    fn nimg_parse_revalidates() {
        let mut ok = b"NIMG".to_vec();
        ok.extend_from_slice(&2u32.to_le_bytes());
        ok.extend_from_slice(&1u32.to_le_bytes());
        ok.extend_from_slice(&[9u8; 8]);
        let (w, h, px) = parse_nimg(&ok, 256).expect("정상 응답");
        assert_eq!((w, h), (2, 1));
        assert_eq!(px, [9u8; 8]);

        assert!(parse_nimg(b"JUNK", 256).is_none(), "서명 불일치");
        let mut short = ok.clone();
        short.pop();
        assert!(parse_nimg(&short, 256).is_none(), "길이 불일치");
        let mut huge = b"NIMG".to_vec();
        huge.extend_from_slice(&10_000u32.to_le_bytes());
        huge.extend_from_slice(&1u32.to_le_bytes());
        assert!(
            parse_nimg(&huge, 256).is_none(),
            "★ 오염된 자식의 크기 주장은 본체가 자른다"
        );
    }
}
