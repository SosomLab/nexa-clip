// winres.rs — Windows 실행 파일 아이콘·버전 리소스·매니페스트 삽입(T-77 보조 · 11-03) · **외부 crate 0**(winres/embed-resource 없이 직접).
//
// 원본 = nexa-sql `packaging/windows/winres.rs`(T-62 · docs/93 §4) 이식. 두 실행 파일(nexa-clip 본체 · nclip-imgdec 워커)의
// build.rs가 `include!`로 함께 쓴다(복사 대신 한 벌).
//
// ★ 왜 — VirusTotal 휴리스틱(choco 설치본 0.1.5 → 10건 초과 · T-78)은 **서명 없음 + 버전 정보 공란 + 매니페스트 없음**인
//   PE를 가장 의심한다. 코드 서명(T-77)이 근본 대응이지만, 탐색기 속성·SmartScreen 평판·winget/ARP가 보는
//   ProductName/FileVersion/CompanyName/OriginalFilename과 `asInvoker` 매니페스트는 서명 없이도 채울 수 있는 몫이다.
//
// - 대상 OS가 windows일 때만 일한다. 다른 OS 산출물은 불변.
// - `packaging/windows/<rc>`(아이콘 = `packaging/branding/nexa-clip.ico` SSOT)를 OUT_DIR의 래퍼 .rc
//   (`#define NCLIP_VER_*` ← `CARGO_PKG_VERSION` → `#include` 원본)로 감싸 컴파일한다 — 버전의 단일 원천 = Cargo.toml.
// - 매니페스트(`packaging/windows/<stem>.manifest`)는 `@VERSION4@`를 치환한 사본을 OUT_DIR에 쓰고 `NCLIP_MANIFEST`로 넘긴다.
// - 컴파일러: MSVC = `rc.exe`(PATH · VS 개발자 프롬프트 `WindowsSdkDir` · Windows Kits 최신 판) 또는 `llvm-rc` /
//   GNU = `windres`. **없으면 조용히 건너뛴다**(아이콘·버전 없는 exe · 기능 동일 · 경고 0 — 크로스 clippy가 시끄럽지 않게).
//   CI(windows-latest)에는 Windows SDK가 있어 릴리스 산출물에는 항상 들어간다.
// - 링크는 `cargo:rustc-link-arg-bins=<.res|.o>` — 그 크레이트의 bin에만.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `rc_name` = packaging/windows 안의 .rc 파일 이름 · `stem` = OUT_DIR 산출물 이름 앞부분(= 매니페스트 파일 이름 앞부분).
fn embed_windows_resources(rc_name: &str, stem: &str) {
    // 런타임 env(빌드 스크립트 규약) — 단독 컴파일·테스트도 가능(env!는 컴파일 시점 고정).
    let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let manifest_dir = PathBuf::from(manifest_dir);
    let win = manifest_dir.join("../../packaging/windows");
    let rc = win.join(rc_name);
    let ico = manifest_dir.join("../../packaging/branding/nexa-clip.ico");
    let shared = win.join("winres.rs");
    let app_manifest = win.join(format!("{stem}.manifest"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", shared.display());
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-changed={}", app_manifest.display());
    println!("cargo:rerun-if-env-changed=WindowsSdkDir");
    println!("cargo:rerun-if-env-changed=WindowsSDKVersion");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // `cargo package`처럼 저장소 밖에서 빌드하면 packaging/이 없다 — 그때도 조용히.
    if !rc.is_file() || !ico.is_file() {
        return;
    }
    let (Ok(out_dir), Ok(target_env)) = (env::var("OUT_DIR"), env::var("CARGO_CFG_TARGET_ENV"))
    else {
        return;
    };
    let out_dir = PathBuf::from(out_dir);
    let manifest_out = write_manifest(&out_dir, &app_manifest, stem);
    let Some(wrapper) = write_wrapper(&out_dir, &rc, stem, manifest_out.as_deref()) else {
        return;
    };
    // 리소스 파일 안의 상대 경로("../branding/nexa-clip.ico")는 원본 .rc 폴더 기준 — cwd를 거기로.
    let Some(cwd) = rc.parent() else {
        return;
    };
    let linked = if target_env == "msvc" {
        compile_msvc(&out_dir, cwd, &wrapper, stem)
    } else {
        compile_gnu(&out_dir, cwd, &wrapper, stem)
    };
    if let Some(obj) = linked {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    }
}

/// Cargo 버전 → (major, minor, patch) · 프리릴리스 꼬리(`-rc.1`)는 뗀다.
fn version_parts() -> Option<(String, u16, u16, u16)> {
    let ver = env::var("CARGO_PKG_VERSION").ok()?;
    let mut nums = ver
        .split('-')
        .next()?
        .split('.')
        .map(|s| s.parse::<u16>().unwrap_or(0));
    let (maj, min, pat) = (
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
    );
    Some((ver, maj, min, pat))
}

/// rc.exe가 읽을 수 있는 절대 경로 문자열(슬래시 · `\\?\` 접두 제거).
fn rc_path(p: &Path) -> Option<String> {
    let s = p.canonicalize().ok()?;
    let s = s.to_string_lossy().replace('\\', "/");
    // Windows canonicalize()는 `\\?\` 접두를 붙인다 — rc.exe가 못 읽으니 뗀다.
    Some(s.strip_prefix("//?/").unwrap_or(&s).to_string())
}

/// 매니페스트 템플릿의 `@VERSION4@`를 `X.Y.Z.0`으로 치환해 OUT_DIR에 쓴다. 템플릿이 없으면 None(매니페스트 없이 진행).
fn write_manifest(out_dir: &Path, template: &Path, stem: &str) -> Option<PathBuf> {
    let text = fs::read_to_string(template).ok()?;
    let (_, maj, min, pat) = version_parts()?;
    let text = text.replace("@VERSION4@", &format!("{maj}.{min}.{pat}.0"));
    let path = out_dir.join(format!("{stem}.manifest"));
    fs::write(&path, text).ok()?;
    Some(path)
}

/// OUT_DIR에 버전 define + `#include` 래퍼를 쓴다. 경로는 슬래시(rc.exe·windres 둘 다 받는다).
fn write_wrapper(
    out_dir: &Path,
    rc: &Path,
    stem: &str,
    manifest: Option<&Path>,
) -> Option<PathBuf> {
    let (ver, maj, min, pat) = version_parts()?;
    let inc = rc_path(rc)?;
    let mut text = format!(
        "// 생성됨 — packaging/windows/winres.rs (버전 단일 원천 = Cargo.toml)\n\
         #define NCLIP_VER_MAJOR {maj}\n#define NCLIP_VER_MINOR {min}\n#define NCLIP_VER_PATCH {pat}\n\
         #define NCLIP_VER_STR \"{ver}\"\n"
    );
    if let Some(m) = manifest.and_then(rc_path) {
        text.push_str(&format!("#define NCLIP_MANIFEST \"{m}\"\n"));
    }
    text.push_str(&format!("#include \"{inc}\"\n"));
    let path = out_dir.join(format!("{stem}-versioned.rc"));
    fs::write(&path, text).ok()?;
    Some(path)
}

/// MSVC: rc.exe(또는 llvm-rc) → .res (link.exe·lld-link가 그대로 받는다).
fn compile_msvc(out_dir: &Path, cwd: &Path, wrapper: &Path, stem: &str) -> Option<PathBuf> {
    let res = out_dir.join(format!("{stem}.res"));
    let tool = find_rc_exe()?;
    let status = Command::new(&tool)
        .current_dir(cwd)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res)
        .arg(wrapper)
        .status()
        .ok()?;
    if status.success() && res.is_file() {
        Some(res)
    } else {
        None
    }
}

/// GNU(mingw): windres → COFF .o.
fn compile_gnu(out_dir: &Path, cwd: &Path, wrapper: &Path, stem: &str) -> Option<PathBuf> {
    let obj = out_dir.join(format!("{stem}-res.o"));
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let prefixed = format!("{arch}-w64-mingw32-windres");
    for tool in ["windres", prefixed.as_str(), "x86_64-w64-mingw32-windres"] {
        let ok = Command::new(tool)
            .current_dir(cwd)
            .args(["-O", "coff", "-o"])
            .arg(&obj)
            .arg(wrapper)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok && obj.is_file() {
            return Some(obj);
        }
    }
    None
}

/// rc.exe 찾기: PATH → `WindowsSdkDir`+`WindowsSDKVersion`(VS 개발자 프롬프트) → Windows Kits 10 최신 bin → llvm-rc(PATH).
fn find_rc_exe() -> Option<PathBuf> {
    let host_sub = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => "x86",
    };
    if let Some(p) = on_path("rc.exe") {
        return Some(p);
    }
    if let (Ok(dir), Ok(ver)) = (env::var("WindowsSdkDir"), env::var("WindowsSDKVersion")) {
        let p = Path::new(&dir)
            .join("bin")
            .join(ver.trim_end_matches('\\'))
            .join(host_sub)
            .join("rc.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    let kits = env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
    let bin = Path::new(&kits).join("Windows Kits").join("10").join("bin");
    if let Ok(rd) = fs::read_dir(&bin) {
        let mut vers: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("10."))
            })
            .collect();
        vers.sort();
        if let Some(p) = vers
            .iter()
            .rev()
            .map(|v| v.join(host_sub).join("rc.exe"))
            .find(|p| p.is_file())
        {
            return Some(p);
        }
    }
    on_path("llvm-rc.exe").or_else(|| on_path("llvm-rc"))
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}
