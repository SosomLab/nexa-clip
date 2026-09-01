//! 저장소 진단 덤프 — 세그먼트의 **이벤트 순서**와 blob 참조 상태를 사람 눈으로.
//!
//! 사용: `cargo run -p nclip-store --example store_dump -- <store 폴더>`
//! (기본: `target/debug/data/store` — 개발 실기 자리)

use nclip_store::diag;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/debug/data/store".into());
    if let Err(e) = diag::dump(std::path::Path::new(&dir)) {
        eprintln!("덤프 실패: {e}");
        std::process::exit(1);
    }
}
