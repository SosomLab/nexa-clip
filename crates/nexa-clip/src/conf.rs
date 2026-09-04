//! 설정 영속(T-12c2) — ★ **저장 버튼이 없는 이유를 끝까지 밀어붙인다**.
//!
//! 설정 화면은 값을 바꾸는 즉시 적용한다([`nclip_ui::SettingsWidget::take_changes`]).
//! 여기서 하는 일은 그 즉시 적용을 **디스크까지 잇는 것**이다 — 사용자는 저장을 누른 적이
//! 없지만 다음 실행에 값이 살아 있어야 한다.
//!
//! ## 무엇을 여기서 하고 무엇을 [`nexa_conf`]가 하는가
//!
//! | 여기(앱) | [`nexa_conf`](nexa_conf)(공용 크레이트) |
//! |---|---|
//! | **어디에 쓸지** 결정 · 시계 주입 · 아는 키 목록 제공 | 포맷 · 미지 키 보존 · 저장 시점 판정 · 원자적 쓰기 |
//!
//! ## ★ 저장을 미루는 이유
//!
//! 슬라이더 하나를 끌면 값 변경이 **수십 번** 난다. 그때마다 쓰면 SSD를 긁는다.
//! [`nexa_conf::SaveScheduler`]가 *"조용해진 뒤 1초"* 또는 *"첫 변경 후 10초"* 중
//! 먼저 오는 쪽에 **한 번만** 쓴다. 중간 상태를 큐에 담지 않으므로 자료구조가 필요 없다.
//!
//! ## ★ 모르는 키를 지우지 않는다
//!
//! 신버전이 쓴 파일을 구버전이 열면, 구버전은 자기가 모르는 키를 만난다. 그걸 버리고
//! 저장하면 **신버전 설정이 조용히 사라진다**. [`nexa_conf::Store::keep_unknown`]으로
//! 되돌려 주면 그대로 재방출된다.

use std::path::{Path, PathBuf};
use std::time::Instant;

use nclip_ui::SettingsState;

/// 설정 파일 이름 — 계열 공통.
const FILE: &str = "settings.cfg";
/// 조용해진 뒤 이만큼 지나면 쓴다.
const QUIET_MS: u64 = 1_000;
/// 연속 변경이라 영영 조용해지지 않아도 이 안에는 반드시 쓴다(starvation 방지).
const MAX_DELAY_MS: u64 = 10_000;

/// 설정이 놓일 폴더 — **포터블 우선**([DR-33](../../../docs/10-decision-record.md)).
///
/// 1. 실행 파일 옆 `data/` — USB에 담아 다니면 설정도 따라온다
/// 2. 사용자 설정 폴더(`%APPDATA%` · `~/Library/Application Support` · `$XDG_CONFIG_HOME`)
/// 3. 현재 폴더 — 둘 다 막힌 극단(그래도 앱은 뜬다)
///
/// ★ **업그레이드 때 폴더째 교체되는 자리는 1을 건너뛴다**
/// ([`nexa_conf::is_replaced_on_upgrade`]). mac `.app` 번들과 Homebrew keg는 **쓰기가 되는데도**
/// 업그레이드가 디렉터리를 갈아 끼워 그 안의 설정이 함께 사라진다 — beep이 실제로 겪은 사고다.
/// ★ 실행 프로필(09-04 사용자 — "인자로 지정해 인스턴스 중복을 피해 실행"): `--profile <이름>`이면
/// 데이터 폴더가 `…/data/profiles/<이름>`으로 갈라져 설정·저장소·신원·기기 목록이 전부 따로다 =
/// 한 PC에서 두 인스턴스가 **서로 다른 기기**로 동기화 시험을 할 수 있다. 단일 인스턴스 가드도 프로필별.
static PROFILE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// 프로필 지정(부팅 1회 · 인자 파싱 직후).
pub(crate) fn set_profile(name: &str) {
    let _ = PROFILE.set(name.to_string());
}

/// 현재 프로필(기본 실행 = `None`).
pub(crate) fn profile() -> Option<&'static str> {
    PROFILE.get().map(String::as_str)
}

/// 프로필 이름 규칙 — 파일 이름·뮤텍스 이름에 그대로 쓰므로 영문·숫자·`-`·`_` 1~32자.
pub(crate) fn valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[must_use]
pub(crate) fn data_dir() -> PathBuf {
    let base = base_data_dir();
    match profile() {
        Some(p) => {
            let dir = base.join("profiles").join(p);
            let _ = std::fs::create_dir_all(&dir);
            dir
        }
        None => base,
    }
}

/// 프로필 없는 기본 데이터 폴더(포터블 = exe 옆 `data` · 아니면 사용자 설정 폴더).
fn base_data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if !nexa_conf::is_replaced_on_upgrade(dir) {
                let data = dir.join("data");
                if nexa_conf::dir_writable(&data) {
                    return data;
                }
            }
        }
    }
    if let Some(dir) = nexa_conf::user_config_dir("nexa-clip") {
        if nexa_conf::dir_writable(&dir) {
            return dir;
        }
    }
    PathBuf::from(".")
}

/// 값(앱 소유) + 파일(크레이트 소유)을 한 손잡이로 묶는다.
#[derive(Debug)]
pub(crate) struct Settings {
    /// 현재 값 — 화면·기능이 읽는 쪽.
    pub(crate) state: SettingsState,
    store: nexa_conf::Store,
    /// 저장이 실패한 적이 있는가 — ★ **같은 오류를 매초 찍지 않기 위해**.
    reported_error: bool,
}

impl Settings {
    /// 표준 위치에서 연다.
    pub(crate) fn load() -> Self {
        Self::open_at(data_dir().join(FILE))
    }

    /// 경로를 지정해 연다(테스트·포터블 강제).
    ///
    /// 파일이 없거나 깨졌어도 **실패하지 않는다** — 기본값으로 뜨는 게 안 뜨는 것보다 낫다.
    pub(crate) fn open_at(path: PathBuf) -> Self {
        let mut state = SettingsState::with_defaults();
        let (mut store, doc) = nexa_conf::Store::open(path, QUIET_MS, MAX_DELAY_MS);
        for (k, v) in doc.pairs {
            // 아는 키면 덮어쓰고, 모르는 키면 되돌려 준다(다음 저장에 그대로 재방출).
            if !state.set_by_name(&k, &v) {
                store.keep_unknown(k, v);
            }
        }
        Self {
            state,
            store,
            reported_error: false,
        }
    }

    /// 저장 경로(콘솔 안내·진단용).
    pub(crate) fn path(&self) -> &Path {
        self.store.path()
    }

    /// 값 변경 — ★ **여기서는 디스크를 만지지 않는다**. 플래그만 세운다.
    pub(crate) fn set(&mut self, key: &'static str, value: String, now: Instant) {
        self.state.set(key, value);
        self.store.sched.mark(now);
    }

    /// 미저장 변경이 있는가(창 유휴 판정에 쓴다).
    pub(crate) fn dirty(&self) -> bool {
        self.store.sched.dirty()
    }

    /// 주기 호출 — 때가 됐으면 쓴다. **쓴 경우에만** `true`.
    pub(crate) fn tick(&mut self, now: Instant) -> bool {
        if self.store.sched.tick(now) {
            self.write()
        } else {
            false
        }
    }

    /// 종료 직전 — 조건을 무시하고 남은 변경을 쓴다.
    ///
    /// ★ **이게 없으면 "바꾸고 바로 닫으면 안 저장됨"** 이 된다(조용해질 1초를 못 기다린다).
    pub(crate) fn flush(&mut self) -> bool {
        if self.store.sched.flush_now() {
            self.write()
        } else {
            false
        }
    }

    fn write(&mut self) -> bool {
        let pairs = self.state.known_pairs();
        match self.store.save(&pairs) {
            Ok(wrote) => {
                self.reported_error = false;
                wrote
            }
            Err(e) => {
                // 실패해도 앱은 계속 돈다 — 값은 메모리에 살아 있다.
                if !self.reported_error {
                    self.reported_error = true;
                    eprintln!("설정 저장 실패: {} — {e}", self.store.path().display());
                    eprintln!("  조치: 폴더 쓰기 권한을 확인하세요. 값은 이 세션 동안 유지됩니다.");
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::time::Duration;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nclip-conf-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("settings.cfg")
    }

    /// ★ 바꾼 값이 다음 실행에 살아 있다 — 이 기능의 존재 이유 그 자체.
    #[test]
    fn value_survives_restart() {
        let p = tmp("restart");
        let t0 = Instant::now();
        {
            let mut s = Settings::open_at(p.clone());
            assert_eq!(s.state.get("ui.theme"), "system", "기본값");
            s.set("ui.theme", "dark".into(), t0);
            assert!(s.flush(), "종료 직전 flush가 쓴다");
        }
        let s = Settings::open_at(p.clone());
        assert_eq!(s.state.get("ui.theme"), "dark", "다시 열면 저장값이 온다");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// ★ 변경 즉시 쓰지 않는다 — 슬라이더 한 번에 수십 번 쓰면 안 된다.
    #[test]
    fn changes_are_coalesced_not_written_immediately() {
        let p = tmp("coalesce");
        let t0 = Instant::now();
        let mut s = Settings::open_at(p.clone());
        for i in 0..10 {
            s.set(
                "store.max_items",
                format!("{}", 100 + i),
                t0 + Duration::from_millis(i * 50),
            );
            assert!(
                !s.tick(t0 + Duration::from_millis(i * 50 + 10)),
                "조용 전엔 안 쓴다"
            );
        }
        assert!(s.dirty());
        assert!(
            s.tick(t0 + Duration::from_millis(2_000)),
            "조용해지면 한 번 쓴다"
        );
        assert!(!s.dirty());
        assert_eq!(s.state.get("store.max_items"), "109", "마지막 값만 남는다");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// ★ 모르는 키를 지우지 않는다 — 구버전이 신버전 설정을 날리면 안 된다.
    #[test]
    fn unknown_keys_are_not_dropped() {
        let p = tmp("unknown");
        std::fs::write(&p, "_schema=1\nfrom.future=keepme\nui.theme=light\n").unwrap();
        let mut s = Settings::open_at(p.clone());
        assert_eq!(s.state.get("ui.theme"), "light");
        s.set("ui.theme", "dark".into(), Instant::now());
        assert!(s.flush());
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("from.future=keepme"), "미지 키가 살아남는다");
        assert!(text.contains("ui.theme=dark"));
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// 깨진 파일에도 앱이 뜬다(기본값으로).
    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let p = tmp("corrupt");
        std::fs::write(&p, "\u{0}\u{1}쓰레기\n===\n").unwrap();
        let s = Settings::open_at(p.clone());
        assert_eq!(s.state.get("ui.theme"), "system");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// 변경이 없으면 flush도 쓰지 않는다(빈 저장 금지).
    #[test]
    fn flush_without_change_writes_nothing() {
        let p = tmp("noop");
        let mut s = Settings::open_at(p.clone());
        assert!(!s.flush());
        assert!(!p.exists(), "건드리지 않은 설정은 파일을 만들지 않는다");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// 표준 위치가 항상 하나로 정해진다(호출마다 흔들리면 저장·조회 루트가 갈린다).
    #[test]
    fn data_dir_is_stable() {
        assert_eq!(data_dir(), data_dir());
    }
}

/// `ui.theme` → 실제 테마(08-30 사용자 요청 "시스템 항목 · 직접 고르지 않으면 시스템 기본").
/// `system`(기본)은 OS 선호를 따르고, OS가 무선호/판정 불가면 **다크**(우리 기본 룩).
/// `dark`/`light`는 명시값 존중.
pub(crate) fn resolve_theme(setting: &str, system_dark: Option<bool>) -> nclip_ctl::theme::Theme {
    use nclip_ctl::theme::Theme;
    match setting {
        "light" => Theme::light(),
        "dark" => Theme::dark(),
        _ => {
            if system_dark == Some(false) {
                Theme::light()
            } else {
                Theme::dark()
            }
        }
    }
}

/// 지금 OS 선호로 `ui.theme`을 푼다.
pub(crate) fn current_theme(setting: &str) -> nclip_ctl::theme::Theme {
    resolve_theme(setting, nclip_plat::theme::system_prefers_dark())
}

#[cfg(test)]
mod theme_tests {
    use super::resolve_theme;

    #[test]
    fn system_follows_os_and_explicit_wins() {
        assert!(!resolve_theme("system", Some(false)).is_dark);
        assert!(resolve_theme("system", Some(true)).is_dark);
        assert!(
            resolve_theme("system", None).is_dark,
            "무선호 = 다크(기본 룩)"
        );
        assert!(!resolve_theme("light", Some(true)).is_dark);
        assert!(resolve_theme("dark", Some(false)).is_dark);
    }
}

/// ★ UI 글꼴 해석(09-01 사용자 요청 "JetBrains Mono") — `ui.font_family`가 있으면
/// 이름으로 시스템 글꼴을 찾고(mmap), 없거나 못 찾으면 시스템 기본. 못 찾은 것은
/// 조용히 넘어가지 않고 콘솔에 알린다(DR-31 — 설정이 거짓말하면 안 된다).
/// 된 폰트를 돌려준다 — ★ **시스템 기본이 항상 폴백으로 붙는다**(09-01 "두부 예방,
/// 설정과 무관하게"). JetBrains Mono처럼 한글이 없는 글꼴을 지정해도 한글은
/// 시스템 본이 받아 UI가 깨지지 않는다(글자 단위 폴백 — nclip-gfx).
pub(crate) fn load_ui_font(conf: &Settings) -> Option<nclip_gfx::Font> {
    use nclip_gfx::Font;
    let sys = nclip_plat::font::system_ui_font();
    let fam = conf.state.get("ui.font_family").trim().to_string();
    // ① 주 글꼴(+ 사용자 지정이면 시스템 본을 첫 폴백으로).
    let mut font = if fam.is_empty() {
        Font::from_static(sys?.0, sys?.1).ok()?
    } else if let Some((d, i)) = nclip_plat::font::find_font_by_family(&fam) {
        println!("UI 글꼴: {fam} (+시스템 폴백)");
        let mut f = Font::from_static(d, i).ok()?;
        if let Some((sd, si)) = sys {
            let _ = f.push_fallback(sd, si);
        }
        f
    } else {
        eprintln!("⚠️ 글꼴 '{fam}'을(를) 못 찾았습니다 — 시스템 기본으로 실행합니다");
        Font::from_static(sys?.0, sys?.1).ok()?
    };
    // ② ★ OS별 기호·이모지 폴백 체인(09-04 사용자 "두부 제거") — 설정과 무관하게 항상 붙는다.
    //   글자 단위 폴백이라 기준선·줄 높이는 주 글꼴이 계속 정한다.
    let mut names = Vec::new();
    for (data, idx, name) in nclip_plat::font::symbol_fallback_fonts() {
        if font.push_fallback(data, idx).is_ok() {
            names.push(name.to_string());
        }
    }
    // ★ 고정폭(Nerd Font) 폴백(09-04) — 평문 행에서도 터미널 아이콘 글리프(PUA)가 두부가 되지 않게.
    if let Some((d, i, name)) = mono_family(conf) {
        if font.push_fallback(d, i).is_ok() {
            names.push(name);
        }
    }
    if names.is_empty() {
        println!("글꼴 폴백: 기호·이모지 본 없음 — 주 글꼴에 없는 기호는 □로 보입니다");
    } else {
        // 진단 — 실기에서 두부가 보이면 이 줄로 어느 본이 빠졌는지 안다.
        println!(
            "글꼴 폴백: {} · 커버 ✓={} 🎉={}",
            names.join(" → "),
            font.covers('\u{2713}'),
            font.covers('\u{1F389}')
        );
    }
    Some(font)
}

/// ★ 고정폭 글꼴 후보(09-04 — 터미널/코드 리치 런): `ui.font_mono`가 있으면 그것, 없으면 OS별 후보 중 첫 설치본.
/// Nerd Font를 앞에 둔다 — 터미널 복사본의 아이콘 글리프(PUA)가 여기서만 산다.
fn mono_family(conf: &Settings) -> Option<(&'static [u8], u32, String)> {
    let want = conf.state.get("ui.font_mono").trim().to_string();
    if !want.is_empty() {
        return match nclip_plat::font::find_font_by_family(&want) {
            Some((d, i)) => Some((d, i, want)),
            None => {
                eprintln!("⚠️ 고정폭 글꼴 '{want}'을(를) 못 찾았습니다 — 기본 후보로 대신합니다");
                mono_default()
            }
        };
    }
    mono_default()
}

fn mono_default() -> Option<(&'static [u8], u32, String)> {
    const CANDIDATES: &[&str] = &[
        "JetBrainsMono Nerd Font",
        "JetBrainsMono NF",
        "CaskaydiaCove Nerd Font",
        "D2Coding",
        "JetBrains Mono",
        "Cascadia Mono",
        "Consolas",
        "Menlo",
        "SF Mono",
        "DejaVu Sans Mono",
        "Noto Sans Mono",
    ];
    CANDIDATES.iter().find_map(|n| {
        nclip_plat::font::find_font_by_family(n).map(|(d, i)| (d, i, (*n).to_string()))
    })
}

/// ★ 고정폭 글꼴 슬롯(09-04) — 주 글꼴을 폴백으로 붙여 한글·기호는 주 글꼴이 받는다. 없으면 None(= 주 글꼴).
pub(crate) fn load_mono_font(conf: &Settings, ui: &nclip_gfx::Font) -> Option<nclip_gfx::Font> {
    let (d, i, name) = mono_family(conf)?;
    let mut f = nclip_gfx::Font::from_static(d, i).ok()?;
    f.push_fallback_font(ui);
    println!("고정폭 글꼴: {name} (+주 글꼴 폴백)");
    Some(f)
}

/// ★ `app.lang` 적용(09-02 "재시작 없는 설정") — 부팅·변경 즉시 전역 언어를 바꾼다.
/// 이전엔 부팅에서조차 안 불려 설정이 죽은 값이었다(스윙 중 발견).
pub(crate) fn apply_lang(conf: &Settings) {
    let code = conf.state.get("app.lang");
    if let Some(l) = nclip_core::Lang::from_code(code) {
        nclip_core::set_lang(l);
    }
}
