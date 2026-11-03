//! build.rs — `nclip-imgdec.exe`의 아이콘·버전·매니페스트 리소스(Windows · T-77 보조 · 11-03).
//! 본체 = `packaging/windows/winres.rs`(nexa-clip과 공용 · 원본 nexa-sql T-62). 다른 OS·rc 도구 없음 = 조용히 건너뛴다.
include!("../../packaging/windows/winres.rs");

fn main() {
    embed_windows_resources("nclip-imgdec.rc", "nclip-imgdec");
}
