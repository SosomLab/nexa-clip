//! 클립보드 **왕복 진단** — 결함 추적용(08-31 사용자 실기: 팝업 원본 붙여넣기에서 서식 유실).
//!
//! 지금 클립보드를 스냅숏(A) → 팝업과 같은 경로(`clipboard::set_reps`)로 재게시 →
//! 다시 스냅숏(B) → 표현별로 비교해 **무엇이 사라지고 무엇이 변했는지** 표로 찍는다.
//!
//! 사용(Windows):
//! 1. 원본 앱(브라우저·Word)에서 문제의 내용을 복사
//! 2. `cargo run -p nclip-plat --example clip_roundtrip`
//! 3. 표 확인 — 이 시점의 클립보드는 **재게시본**이므로, 이어서 대상 앱에 `Ctrl+V` 하면
//!    팝업 Enter와 같은 조건으로 증상이 재현된다
//!
//! 각 표현의 원본 바이트는 `%TEMP%\nclip-roundtrip\`에 저장된다(내용 비교용).

#[cfg(windows)]
fn main() {
    use nclip_plat::{clipboard, watch_win};

    let Some(a) = watch_win::read_snapshot() else {
        eprintln!("클립보드를 읽지 못했습니다");
        std::process::exit(1);
    };
    println!(
        "A: 표현 {}개 · 출처 {} · seq {}",
        a.reps.len(),
        a.source_app.as_deref().unwrap_or("?"),
        a.seq
    );

    let dir = std::env::temp_dir().join("nclip-roundtrip");
    let _ = std::fs::create_dir_all(&dir);
    for (i, r) in a.reps.iter().enumerate() {
        let safe: String = r
            .format
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let _ = std::fs::write(dir.join(format!("A-{i:02}-{safe}.bin")), &r.data);
    }

    // `--no-ole`: 주인 잃은 OLE 사설 포맷을 빼고 재게시(원본 앱의 데이터 객체를
    // 참조하는 장부라 재게시 시점엔 죽은 참조다 — Word가 OLE 경로를 타다 실패하면
    // 서식이 갓는 경로로 안 간다는 가설 검증 · 08-31 결함 2).
    const OLE_PRIVATE: [&str; 9] = [
        "DataObject",
        "Ole Private Data",
        "OwnerLink",
        "ObjectLink",
        "Link Source",
        "Link Source Descriptor",
        "Embed Source",
        "Native",
        "Object Descriptor",
    ];
    let no_ole = std::env::args().any(|a| a == "--no-ole");
    let post: Vec<nclip_core::RawRep> = if no_ole {
        let kept: Vec<nclip_core::RawRep> = a
            .reps
            .iter()
            .filter(|r| !OLE_PRIVATE.contains(&r.format.as_str()))
            .cloned()
            .collect();
        println!(
            "--no-ole: {} → {}개로 줄여 재게시",
            a.reps.len(),
            kept.len()
        );
        kept
    } else {
        a.reps.clone()
    };

    match clipboard::set_reps(&post) {
        Ok(n) => println!("재게시: {n}개 (팝업 Enter와 같은 경로)"),
        Err(e) => {
            eprintln!("재게시 실패: {e}");
            std::process::exit(1);
        }
    }

    let Some(b) = watch_win::read_snapshot() else {
        eprintln!("재게시본을 읽지 못했습니다");
        std::process::exit(1);
    };
    for (i, r) in b.reps.iter().enumerate() {
        let safe: String = r
            .format
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let _ = std::fs::write(dir.join(format!("B-{i:02}-{safe}.bin")), &r.data);
    }

    // 비교 — 이름으로 짝을 짓는다(같은 이름이 여럿이면 등장 순).
    println!("\n{:-^72}", " A(원본) → B(재게시) ");
    let mut used = vec![false; b.reps.len()];
    for ra in &a.reps {
        let hit = b
            .reps
            .iter()
            .enumerate()
            .find(|(j, rb)| !used[*j] && rb.format == ra.format);
        match hit {
            Some((j, rb)) => {
                used[j] = true;
                let verdict = if ra.data == rb.data {
                    "= 동일".to_string()
                } else {
                    let diff_at = ra
                        .data
                        .iter()
                        .zip(rb.data.iter())
                        .position(|(x, y)| x != y)
                        .unwrap_or_else(|| ra.data.len().min(rb.data.len()));
                    format!("★ 다름 (첫 차이 오프셋 {diff_at})")
                };
                println!(
                    "{:<44} {:>8}B → {:>8}B  {}",
                    ra.format,
                    ra.data.len(),
                    rb.data.len(),
                    verdict
                );
            }
            None => println!(
                "{:<44} {:>8}B → {:>8}   ★ B에 없음(유실)",
                ra.format,
                ra.data.len(),
                "—"
            ),
        }
    }
    for (j, rb) in b.reps.iter().enumerate() {
        if !used[j] {
            println!(
                "{:<44} {:>8} → {:>8}B  ★ B에만 있음",
                rb.format,
                "—",
                rb.data.len()
            );
        }
    }
    println!("\n바이트 저장: {}", dir.display());
    println!("지금 클립보드 = 재게시본 — 대상 앱에 Ctrl+V로 증상을 재현해 보세요.");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("이 진단은 Windows 전용입니다(결함이 Windows에서 보고됨).");
}
