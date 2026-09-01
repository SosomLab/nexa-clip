//! sealed 봉투 처리량 측정(09-01 — dev 프로필 opt 적용 확인).
fn main() {
    let secret = [7u8; 32];
    let data = vec![0xA5u8; 8 * 1024 * 1024];
    let t0 = std::time::Instant::now();
    let sealed = nclip_store::sealed::seal(b"bench", &secret, &data).unwrap();
    let t_seal = t0.elapsed();
    let t1 = std::time::Instant::now();
    let out = nclip_store::sealed::open(b"bench", &secret, &sealed).unwrap();
    println!(
        "seal 8MB: {:?} ({:.0}MB/s) · open: {:?} ({:.0}MB/s) · ok={}",
        t_seal,
        8.0 / t_seal.as_secs_f64(),
        t1.elapsed(),
        8.0 / t1.elapsed().as_secs_f64(),
        out.len() == data.len()
    );
}
