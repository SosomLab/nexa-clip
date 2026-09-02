//! 미리보기 디코드 프로브(09-02 실기 진단) — 현재 클립보드 한 벌을 읽어
//! K4 미리보기와 같은 체인(`thumbnail_source` → DIB/PNG 디코드 → 축소)을
//! 48px(썸네일)·1600px(미리보기) 두 상한으로 돌리고 매 단계를 찍는다.
//!
//! 실행: `cargo run -p nclip-plat --example preview_probe`
//! ⚠️ PNG 경로는 워커(`nclip-imgdec.exe`)가 실행 파일 옆에 있어야 한다 —
//! 예제는 `target/debug/examples/`에서 돌므로 워커를 그 옆에 복사해 시험한다.

fn main() {
    // 인자로 EMF 파일을 주면 해당 파일만 래스터화 시험(09-02 — GDI FFI 검증).
    #[cfg(target_os = "windows")]
    if let Some(path) = std::env::args().nth(1) {
        let bytes = std::fs::read(&path).expect("EMF 읽기");
        println!("EMF {}B", bytes.len());
        for side in [160u32, 1600] {
            match nclip_plat::emf::emf_to_rgba(&bytes, side) {
                Some((w, h, px)) => {
                    let non_white = px
                        .chunks_exact(4)
                        .filter(|p| p[0] != 255 || p[1] != 255 || p[2] != 255)
                        .count();
                    println!("side {side:>4} → {w}×{h} · 비백색 화소 {non_white}");
                }
                None => println!("side {side:>4} → ★ 실패"),
            }
        }
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let Some(snap) = nclip_plat::watch_win::read_snapshot() else {
            eprintln!("클립보드 읽기 실패");
            return;
        };
        println!("표현 {}개:", snap.reps.len());
        for r in &snap.reps {
            println!("  {:28} {:>9}B", r.format, r.data.len());
        }
        let Some(i) = nclip_core::capture::thumbnail_source(&snap.reps) else {
            println!("thumbnail_source = None (이미지 표현 없음)");
            return;
        };
        let r = &snap.reps[i];
        println!("thumbnail_source → [{i}] {}", r.format);
        for side in [48u32, 1600] {
            let out = decode(r, side);
            match out {
                Some((w, h, px)) => println!("side {side:>4} → {w}×{h} ({}B)", px.len()),
                None => println!("side {side:>4} → ★ 실패(None)"),
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn decode(r: &nclip_core::RawRep, side: u32) -> Option<(u32, u32, Vec<u8>)> {
    use nclip_core::img::{dib_to_rgba, downscale_rgba};
    match r.format.as_str() {
        "CF_DIB" | "CF_DIBV5" => {
            let (w, h, rgba) = dib_to_rgba(&r.data)?;
            println!("  dib_to_rgba → {w}×{h}");
            downscale_rgba(w, h, &rgba, side)
        }
        "image/bmp" if r.data.len() > 14 => {
            let (w, h, rgba) = dib_to_rgba(&r.data[14..])?;
            downscale_rgba(w, h, &rgba, side)
        }
        "CF_ENHMETAFILE" if !r.data.is_empty() => {
            let out = nclip_plat::emf::emf_to_rgba(&r.data, side);
            if out.is_none() {
                eprintln!("  emf_to_rgba 실패");
            }
            out
        }
        "PNG" | "public.png" | "image/png" => {
            let out = nclip_plat::imgdec::decode_isolated(&r.data, side);
            if out.is_none() {
                eprintln!("  decode_isolated 실패(워커 경로/디코드)");
            }
            out
        }
        _ => {
            println!("  디코드 불가 포맷");
            None
        }
    }
}
