//! 항목 덤프 — 라벨 부분 일치 항목의 표현(이름·크기·blob 참조)을 찍고 바이트를 파일로 쓴다(09-04 mac 실기 진단:
//! "받은 이미지가 [이미지]로만 보임"). 미적재 blob은 `read_blob_by_id`로 채운다.
//!
//! 사용: `cargo run -p nclip-store --example dump_item -- <store 폴더> <라벨 부분> [출력 폴더]`
use nclip_store::{FileStore, HistoryStore as _};

fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().unwrap_or_else(|| "target/debug/data/store".into());
    let needle = a.next().unwrap_or_default();
    let out = std::path::PathBuf::from(a.next().unwrap_or_else(|| "/tmp/nclip-dump".into()));
    let _ = std::fs::create_dir_all(&out);
    let mut s = match FileStore::open(std::path::Path::new(&dir)) {
        Ok(r) => r.store,
        Err(e) => {
            eprintln!("열기 실패: {e}");
            std::process::exit(1);
        }
    };
    for it in s.load() {
        if !needle.is_empty() && !it.label.contains(&needle) {
            continue;
        }
        println!(
            "#{} {:?} \"{}\" 출처={:?} 표현 {}개 blob {}개",
            it.id,
            it.kind,
            it.label,
            it.source_app,
            it.reps.len(),
            it.blobs.len()
        );
        for (i, r) in it.reps.iter().enumerate() {
            let data = if r.data.is_empty() {
                it.blobs
                    .iter()
                    .find(|(ri, _, _)| *ri as usize == i)
                    .and_then(|(_, id, _)| s.read_blob_by_id(id))
                    .unwrap_or_default()
            } else {
                r.data.clone()
            };
            let head: Vec<String> = data.iter().take(8).map(|b| format!("{b:02x}")).collect();
            println!(
                "   [{i}] {:<40} {:>8}B  head={}",
                r.format,
                data.len(),
                head.join(" ")
            );
            let safe: String = r
                .format
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();
            let _ = std::fs::write(out.join(format!("{}-{i}-{safe}.bin", it.id)), &data);
        }
    }
    println!("→ {}", out.display());
}
