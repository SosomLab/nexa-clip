//! 진단 덤프 — 세그먼트 이벤트를 순서대로 사람 눈에(내용은 안 보여준다 — 라벨·크기만).
//!
//! `store_dump` 예제가 부른다. 결함 추적용이라 형식 안정성은 약속하지 않는다.

use std::path::Path;

use crate::{codec, keys, sealed, DOMAIN_IDX, EV_ADD, EV_REMOVE, EV_TOUCH};

/// 세그먼트를 순서대로 해독해 이벤트 목록·blob 참조를 찍는다.
///
/// # Errors
/// 키를 못 열면(기기 키 불일치 포함) 에러.
pub fn dump(dir: &Path) -> std::io::Result<()> {
    let keys::MasterLoad::Ready(master) = keys::load_master(dir)? else {
        return Err(std::io::Error::other(
            "기기 키 불일치 — 이 폴더의 keys를 열 수 없다",
        ));
    };
    let mut segs: Vec<_> = std::fs::read_dir(dir.join("index"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "idx"))
        .collect();
    segs.sort();
    let mut n = 0usize;
    for seg in segs {
        println!("── {}", seg.display());
        let bytes = std::fs::read(&seg)?;
        let mut off = 0usize;
        while off + 4 <= bytes.len() {
            let len = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap_or([0; 4])) as usize;
            let Some(rec) = bytes.get(off + 4..off + 4 + len) else {
                println!("  [잘린 꼬리 @ {off}]");
                break;
            };
            off += 4 + len;
            n += 1;
            let Some(plain) = sealed::open(DOMAIN_IDX, &master, rec) else {
                println!("  #{n} [봉투 안 열림 — {len}B]");
                continue;
            };
            let mut r = codec::R(&plain);
            match r.u8() {
                Some(t) if t == EV_ADD => {
                    let id = r.u64().unwrap_or(0);
                    let _kind = r.u8();
                    let copies = r.u32().unwrap_or(0);
                    let pinned = r.u8().unwrap_or(0);
                    let _src = r.opt_str();
                    let label = r.str().unwrap_or_default();
                    let thumb = match r.u8() {
                        Some(1) => {
                            let w = r.u32().unwrap_or(0);
                            let h = r.u32().unwrap_or(0);
                            let _ = r.bytes();
                            format!("{w}x{h}")
                        }
                        _ => "-".into(),
                    };
                    let reps = r.u32().unwrap_or(0) as usize;
                    let mut parts = Vec::new();
                    for _ in 0..reps {
                        let Some(f) = r.str() else { break };
                        match r.u8() {
                            Some(1) => {
                                let (idb, rest) = r.0.split_at_checked(32).unwrap_or((&[], r.0));
                                r.0 = rest;
                                let blen = r.u64().unwrap_or(0);
                                let hex: String =
                                    idb.iter().take(4).map(|b| format!("{b:02x}")).collect();
                                let path = {
                                    let full: String =
                                        idb.iter().map(|b| format!("{b:02x}")).collect();
                                    dir.join("blob").join(&full[..2.min(full.len())]).join(full)
                                };
                                let live = if path.exists() { "" } else { " ★없음" };
                                parts.push(format!("{f}=blob:{hex}…({blen}B{live})"));
                            }
                            _ => {
                                let b = r.bytes().map_or(0, <[u8]>::len);
                                parts.push(format!("{f}={b}B"));
                            }
                        }
                    }
                    println!(
                        "  #{n} ADD    id={id} copies={copies} pin={pinned} thumb={thumb} \"{}\" [{}]",
                        label.chars().take(16).collect::<String>(),
                        parts.join(" · ")
                    );
                }
                Some(t) if t == EV_TOUCH => {
                    println!("  #{n} TOUCH  id={}", r.u64().unwrap_or(0));
                }
                Some(t) if t == EV_REMOVE => {
                    println!("  #{n} REMOVE id={}", r.u64().unwrap_or(0));
                }
                other => println!("  #{n} ?{other:?}"),
            }
        }
    }
    Ok(())
}
