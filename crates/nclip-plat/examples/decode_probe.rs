//! 이미지 파일 격리 디코드 프로브(3-OS · 09-04 mac 실기 — 받은 팔레트 PNG가 "[이미지]"로만 보임).
//! 사용: `cargo run -p nclip-plat --example decode_probe -- <파일> [최대 변]`
//! ⚠️ 워커 `nclip-imgdec`가 예제 실행 파일 옆(`target/<profile>/examples/`)에 있어야 한다.
fn main() {
    let path = std::env::args().nth(1).expect("파일 경로");
    let side: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1600);
    let bytes = std::fs::read(&path).expect("파일 읽기");
    println!(
        "{} — {}B · head {:02x?}",
        path,
        bytes.len(),
        &bytes[..bytes.len().min(8)]
    );
    match nclip_plat::imgdec::decode_isolated(&bytes, side) {
        Some((w, h, px)) => {
            let opaque = px.chunks_exact(4).filter(|p| p[3] == 255).count();
            println!(
                "side {side} → {w}×{h} · RGBA {}B · 불투명 화소 {opaque}",
                px.len()
            );
        }
        None => {
            println!("★ 디코드 실패(None)");
            std::process::exit(1);
        }
    }
}
