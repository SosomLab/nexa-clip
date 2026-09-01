//! 같은 라벨 인접 항목의 표현 바이트 차이 — J6(PPT 증식) 원인 포맷 특정(09-01).
#![allow(clippy::unwrap_used)]
use nclip_store::{FileStore, HistoryStore as _};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/debug/data/store".into());
    let mut s = FileStore::open(std::path::Path::new(&dir)).unwrap().store;
    let items = s.load();
    for w in items.windows(2) {
        let (a, b) = (&w[1], &w[0]); // 오래된 것 → 새것
        if a.label != b.label {
            continue;
        }
        if a.reps.len() != b.reps.len() {
            let fa: Vec<&str> = a.reps.iter().map(|r| r.format.as_str()).collect();
            let fb: Vec<&str> = b.reps.iter().map(|r| r.format.as_str()).collect();
            println!(
                "\"{}\" id {}→{} 표현 개수 다름: [{}] vs [{}] (src {:?}→{:?})",
                a.label,
                a.id,
                b.id,
                fa.join(", "),
                fb.join(", "),
                a.source_app,
                b.source_app
            );
            continue;
        }
        let mut diffs = Vec::new();
        for (ra, rb) in a.reps.iter().zip(&b.reps) {
            if ra.format != rb.format {
                diffs.push(format!("순서 다름: {} vs {}", ra.format, rb.format));
            } else if ra.data != rb.data {
                diffs.push(format!("{} ({}B)", ra.format, ra.data.len()));
            }
        }
        if !diffs.is_empty() {
            println!(
                "\"{}\" id {}→{} 차이: {}",
                a.label,
                a.id,
                b.id,
                diffs.join(" · ")
            );
            return; // 한 쌍이면 충분
        }
    }
    println!("차이 나는 인접 동라벨 쌍 없음");
}
