//! 복원 시간 측정 — 시작 블록 논쟁의 사실 확인용(09-01).
use nclip_store::{FileStore, HistoryStore as _};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/debug/data/store".into());
    let t0 = std::time::Instant::now();
    let mut s = match FileStore::open(std::path::Path::new(&dir)) {
        Ok(r) => r.store,
        Err(e) => {
            eprintln!("열기 실패: {e}");
            std::process::exit(1);
        }
    };
    let t_open = t0.elapsed();
    let items = s.load();
    let bytes: usize = items
        .iter()
        .flat_map(|i| i.reps.iter())
        .map(|r| r.data.len())
        .sum();
    println!(
        "open {:?} · load {:?} · {}개 · 표현 합계 {:.1}MB",
        t_open,
        t0.elapsed() - t_open,
        items.len(),
        bytes as f64 / 1e6
    );
}
