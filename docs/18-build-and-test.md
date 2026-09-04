# 18 · 빌드 · 테스트 — 절차 SSOT

> ★ **이 문서가 빌드·테스트 절차의 유일한 원천이다.** 절차를 바꾼 **그 커밋에서 함께 고친다**([16 §2-4](16-doc-git-conventions.md)) — 사후 정리 금지.
>
> 사람이 눈으로 봐야 하는 실기 점검은 여기가 아니라 [21 실기 점검표](21-manual-test.md)에 있다.
> **여기는 기계가 하는 것**, 거기는 **사람이 하는 것**이다.

---

## 1. 준비

| 항목 | 값 | 비고 |
|---|---|---|
| 툴체인 | **stable**(`rust-toolchain.toml`이 고정) | 별도 설치 불필요 — `cargo`가 자동으로 맞춘다 |
| 최소 버전 | `rust-version = "1.82"` | 워크스페이스 공통 |
| 구성 요소 | `rustfmt` · `clippy` | 툴체인 파일에 포함 |
| 외부 도구 | ★ **C 링커(`cc`)가 필요하다** | 정정 08-29 — 아래 참조 |

```bash
git clone git@github.com:SosomLab/nexa-clip.git
cd nexa-clip
cargo build          # 첫 빌드에서 툴체인·의존을 받는다
```

> ⚠️ **정정(08-29)** — 예전 이 표는 *"외부 도구 없음 · C 툴체인이 필요 없다"* 였다. **틀렸다.**
> `rustc`는 링크를 **`cc`에 위임**하므로 C 컴파일러가 없으면 `error: linker \`cc\` not found` 로 죽는다.
> 우리 코드가 C를 안 쓴다는 것과 **툴체인이 필요 없다는 것은 다르다**.
>
> - Windows·macOS는 사실상 항상 있다(MSVC 빌드 도구 · Xcode CLT).
> - **Linux는 없을 수 있다** — 이 함정을 실제로 밟았다(신규 Ubuntu 26.04 데스크톱).
> - ★ **크로스 검사도 마찬가지다** — `--target x86_64-pc-windows-msvc` 로 `cargo check` 만 해도
>   `libc`·`proc-macro2`·`crossbeam-utils`의 **빌드 스크립트가 호스트에서 링크**되므로 `cc`가 필요하다.
>   링커 없이 되는 건 `cargo fmt` 뿐이다.
>
> ★ **Linux 개발·실행·테스트 환경 전체는 [§9](#9-linux-환경--배포판별-차이-포함)** 에 있다.

> ⚠️ **Linux 데스크톱 빌드**는 창(winit)이 X11/Wayland 개발 헤더를 요구할 수 있다.
> 코어만 검사할 때는 [§4-2](#4-2-크레이트만-골라-검사)로 창 없는 크레이트만 돌린다.

---

## 2. ★ 매번 돌리는 네 줄

**커밋 전에 이 순서로 돌린다.** CI가 검사하는 것과 **정확히 같다**([§6](#6-ci--main은-항상-green)).

```bash
cargo fmt --all --check                              # ① 서식
cargo clippy --workspace --all-targets -- -D warnings # ② 린트(경고 = 실패)
cargo test --workspace                               # ③ 테스트
cargo run -p nexa-clip                               # ④ 환경 점검(눈으로 1초)
```

| # | 무엇을 잡나 | 실패하면 |
|:--:|---|---|
| **①** | 서식 흔들림 | `cargo fmt --all`(`--check` 없이) 로 고친다 |
| **②** | ★ **경고를 오류로** 취급 — 미사용·불필요한 분기·의심스러운 패턴 | 고친다. **`allow`로 덮지 않는다**(정당한 예외는 이유를 주석에) |
| **③** | 로직 회귀 | [§3](#3-테스트가-지키는-것)에서 무엇이 깨졌는지 본다 |
| **④** | ★ **이 PC에서 무엇이 되고 무엇이 안 되는지** | 출력이 곧 진단이다([§5](#5-실행--무엇을-확인할-수-있나)) |

> ★ **②를 `-D warnings`로 두는 이유** — 경고를 허용하면 **경고가 쌓이고, 쌓이면 아무도 안 본다**.
> 지금은 0이고, 0을 유지하는 비용이 가장 싸다.

---

## 3. 테스트가 지키는 것

`cargo test --workspace` — 현재 **195개**.

### 3-1. 어디에 무엇이 있나

| 크레이트 | 지키는 것 |
|---|---|
| **`nclip-core`** | i18n 카탈로그 **빈 칸 없음**(4언어) · 열 번호와 `ALL` 순서 일치 · 항목 **중복 키**(순서 무관) · 평문 폴백 존재 · 진단 로그 링 버퍼·**원인/조치 쌍** |
| **`nclip-ctl`** | ★ **디자인 토큰** — 간격이 4의 배수 · 상태 오버레이 **단조 증가** · 그림자 **두 겹** · 팝업 모션 **120ms 상한** · 보기 모드 밀도 순서 |
| **`nclip-plat`** | 감시 게이트 **판정 순서**(민감 표식 > 일시정지 > 다음 1건) · 붙여넣기 능력 진단 |
| **`nclip-ui`** | 설정 **값 키 중복 금지** · 모든 항목이 기본값을 냄 · ★ **팝업 기본값 = 커서**(DR-24) |
| **`nclip-gfx`** | 알파 블렌드 값 · 표면 밖 클리핑에서 **패닉 없음** |

### 3-2. ★ 이 테스트들이 왜 있는가

**"조용히 깨지는 것"만 골라 고정했다.** 컴파일러가 못 잡고 화면에서도 안 보이는 것들:

| 테스트 | 없으면 생기는 일 |
|---|---|
| i18n 빈 칸 검사 | **화면이 비거나 영어로 샌다** — 한 언어만 빠뜨려도 모른다 |
| 설정 **값 키 중복** | **한 설정이 다른 설정을 덮어쓴다** — 화면은 멀쩡해 보인다 |
| 감시 게이트 **순서** | 일시정지 중이면 민감 표식을 건너뛸 수 있다(fail-open) |
| 상태 오버레이 **단조성** | 눌린 게 덜 눌려 보인다 |
| 팝업 모션 **상한** | *"부드럽게"* 가 *"느리게"* 가 된다([00 §2 ①](00-vision.md)) |
| `dedup_key` **순서 무관** | 같은 내용이 표현 순서만 달라도 **새 항목으로 쌓인다** |

### 3-3. 한 개만 돌리기

```bash
cargo test -p nclip-core                    # 크레이트 하나
cargo test -p nclip-core catalog            # 이름에 catalog가 든 것만
cargo test -p nclip-ui -- --nocapture       # println! 보기
```

---

## 4. 교차 검사

### 4-1. 다른 OS 코드가 깨지지 않았는지

★ **한 OS에서 green이어도 반대편은 미검증이다.** `cfg`로 갈린 코드가 조용히 썩는다.

```bash
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin x86_64-unknown-linux-gnu

cargo check --workspace --all-targets --target x86_64-pc-windows-msvc
cargo check --workspace --all-targets --target aarch64-apple-darwin
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu
```

> ⚠️ **`check`는 링크를 하지 않는다** — 타입은 보지만 **OS 라이브러리 링크 오류는 못 잡는다**.
> 실제 링크는 [§6](#6-ci--main은-항상-green) CI의 3-OS 러너가 본다.

### 4-2. 크레이트만 골라 검사

창(winit)을 뺀 순수 계층만 볼 때:

```bash
cargo test -p nclip-core -p nclip-gfx -p nclip-ctl -p nclip-store
```

### 4-3. 릴리스 프로필

```bash
cargo build --release
```

`[profile.release]` = `opt-level="z"` · `lto="fat"` · `codegen-units=1` · `panic="abort"` · `strip`.
★ **크기 우선**이다 — 24시간 상주 앱이라 [DR-9 예산 게이트](10-decision-record.md)가 걸린다(수치는 ADR-0001에서 확정 예정).

---

## 5. 실행 — 무엇을 확인할 수 있나

```bash
cargo run -p nexa-clip                  # 환경 점검
cargo run -p nexa-clip -- demo          # 렌더 데모(1/2/3 보기 · P 반투명 · T 테마)
cargo run -p nexa-clip -- settings      # 설정 창(검색 · 스플리터)
cargo run -p nexa-clip -- spike-paste   # K-1 스파이크(포커스 복원 + 키 주입)
cargo run -p nexa-clip -- --help
```

### 5-1. 환경 점검 읽는 법

```
Nexa Clip v0.1.0
target          : windows
clipboard watch : unavailable (NotImplemented)   ← 아직 감시 미구현(T-14)
paste inject    : ok (win32-sendinput)           ← K-1 경로 사용 가능
default view    : Compact (compact)
sync            : End-to-end encrypted in transit
status          : Local only
```

★ **"안 됨"과 "켜면 됨"을 구분해 찍는다** — macOS에서 `needs permission`이 뜨면
기능이 없는 게 아니라 **손쉬운 사용 권한만 켜면 된다**([`PasteCapability`](../crates/nclip-core/src/paste.rs)).

### 5-2. ⚠️ 스파이크 결과를 오독하지 않기

`spike-paste`에서 **`[3] 포커스 탈취: 실패`** 가 뜨면:

- 대상 앱이 **계속 포그라운드로 남아 있다** → `SetForegroundWindow`가 사실상 할 일이 없다
- 따라서 **주입은 검증되지만 복원 경로(`AttachThreadInput`)는 검증되지 않는다**
- 프로그램이 그 사실을 출력에 명시한다. **"붙여넣기 됐으니 통과"로 넘기지 말 것**

(Windows Terminal 등에서 `GetConsoleWindow`가 0이면 임시 최상위 창으로 탈취를 재시도한다.)

---

## 6. CI — "main은 항상 green"

`.github/workflows/ci.yml` — push(main)·PR에서 돈다.

| 축 | 값 |
|---|---|
| OS | **windows-latest · macos-latest · ubuntu-latest** |
| 단계 | `cargo fmt --all --check` → `clippy -D warnings` → `cargo test --workspace` |
| 캐시 | `Swatinem/rust-cache` |

★ **로컬 [§2](#2--매번-돌리는-네-줄)와 CI가 같은 명령을 쓴다** — 다르면 *"내 PC에선 됐는데"* 가 생긴다.

---

## 7. 흔한 실패와 처방

| 증상 | 원인 | 처방 |
|---|---|---|
| `cargo fmt --all --check` 실패 | 서식 흔들림 | `cargo fmt --all` |
| clippy `unreachable_pub` | 비공개 모듈 안에서 `pub` | `pub(super)` / `pub(crate)` |
| clippy `dead_code` | 이식했지만 아직 안 쓰는 상수·함수 | ★ **지우거나 주석으로 남긴다.** `allow`로 덮지 않는다 |
| clippy `this assertion has a constant value` | 상수 비교 `assert!` | `const { assert!(..) }` 또는 `const _: () = assert!(..)` |
| `failed to remove file nexa-clip.exe` | ★ **실행 중인 바이너리를 덮어쓰려 함** | 그 창을 닫고 다시 빌드 |
| Linux에서 창 관련 링크 오류 | X11/Wayland 개발 패키지 부재 | [§4-2](#4-2-크레이트만-골라-검사)로 코어만 검사 |
| 한글이 네모로 | 시스템 UI 폰트 후보 미스 | 실행 시 콘솔의 `폰트:` 줄 확인 → [`nclip-plat/src/font.rs`](../crates/nclip-plat/src/font.rs) 후보 |

---

## 8. 새 코드를 더할 때의 규율

| # | 규칙 | 왜 |
|:--:|---|---|
| **B-1** | ★ **"조용히 깨지는 것"에 테스트를 붙인다** — 화면에서 안 보이고 컴파일러도 못 잡는 것([§3-2](#3-2--이-테스트들이-왜-있는가)) | 나머지는 실기가 잡는다 |
| **B-2** | **외부 crate 추가는 [10 §3 원장](10-decision-record.md)에 근거와 함께** | DR-8 — 왜 자체 구현이 비현실적인지를 적는다 |
| **B-3** | **OS 분기 코드는 반대편도 `cargo check`** ([§4-1](#4-1-다른-os-코드가-깨지지-않았는지)) | 한쪽만 green인 채로 썩는다 |
| **B-4** | ★ **서버·와이어에 영향이 있으면 [22 전달 원장](22-upstream-beep-liaison.md)** | beep과 공유하는 영역이다 |
| **B-5** | 사람이 봐야 하는 것은 [21 실기 점검표](21-manual-test.md)에 항목으로 남긴다 | ⏳(점검 요청)과 ✅을 구분한다 |

---

## 9. Linux 환경 — 배포판별 차이 포함

> ★ **왜 이 절이 따로 있나** — Windows·macOS는 개발기가 곧 실행기라 환경이 저절로 갖춰진다.
> Linux는 **배포판 · 데스크톱 · 표시 서버 · 설치된 도구**가 전부 갈려서, 같은 바이너리가
> 다른 결과를 낸다. 08-29에 신규 Ubuntu 26.04에서 이 절을 실측으로 만들었다.

### 9-1. 세 가지 환경을 구분한다

| 환경 | 무엇이 필요한가 | 없으면 |
|---|---|---|
| **개발(빌드)** | `rustup` 툴체인 + ★ **C 링커(`cc`)** | `cargo build`/`test`/`clippy` 전부 실패([§1](#1-준비)) |
| **실행** | 표시 서버(Wayland/X11)뿐 — ★ **클립보드는 내재화**(09-03 · x11rb 직결 · 도구 불요). 기능별 선택 패키지는 [§9-1b](#9-1b-기능별-시스템-패키지--무엇이-없으면-무엇이-안-되나) | 헤드리스면 감시가 `NoDisplayServer`로 **정직하게 거부**한다 |
| **테스트(자동)** | 위 둘 + `coreutils` | 순수부 테스트는 링커만 있으면 돈다 |

⚠️ **실행 환경이 개발 환경의 부분집합이 아니다** — 빌드는 되는데 감시가 안 되는 조합이
정상적으로 존재한다(헤드리스 CI가 정확히 그렇다). 그래서 감시 능력은 **런타임 판정**이다.

### 9-1b. ★ 기능별 시스템 패키지 — 무엇이 없으면 무엇이 안 되나

> 09-03 내재화(T-14 본편 · [29 §6](29-linux-clipboard-access.md)) 이후의 진실. **핵심 기능은 전부 무설치**이고,
> 없으면 각 기능이 **정직 강등**한다(기동 실패 없음 · 시작 로그가 사유를 찍는다).

| 기능 | 필요 패키지 | 없으면 |
|---|---|---|
| ★ **클립보드 수집·재적재** (본편) | **없음** — x11rb가 X11/XWayland에 직결 | X 연결 불가 환경에서만 아래 폴백으로 |
| └ 폴백(도구 파이프) | `wl-clipboard`(data-control 컴포지터 — KWin·Sway) · `xclip`(x11rb 연결 실패 시) | `MissingTool` 정직 거부 — 트레이만 동작 |
| 키 주입(K-1 붙여넣기) | `xdg-desktop-portal` + RemoteDesktop 백엔드(GNOME·KDE 기본 탑재) | 클립보드 적재까지만(Ctrl+V는 직접) |
| 전역 단축키(⇧Ctrl+V) | `xdg-desktop-portal` GlobalShortcuts(GNOME 48+·KDE 기본) | 단축키 없음 — 트레이 좌클릭으로 |
| 트레이(SNI) | GNOME: **AppIndicator 확장** · KDE·XFCE: 기본 | 트레이 부재 — 기동 시 힌트 출력 |
| 테마 시스템 추종 | `xdg-desktop-portal` Settings | `ui.theme=system` 불가 — 수동 테마 |
| ★ **최상위 고정 · 창 위치 기억** | **`libxkbcommon-x11-0`** — X11 창 백엔드(winit)가 dlopen. Wayland엔 "항상 위" 프로토콜이 없어 창만 XWayland로 띄우는 구조(09-02) | 정직 강등 — Wayland 창 · 토글 무시 + 설치 안내 로그 |

### 9-2. 설치 — 배포판별 명령

빌드 도구와 클립보드 도구는 이름이 배포판마다 다르다.

| 배포판 계열 | C 툴체인(빌드) | 최상위 고정(선택) | 클립보드 도구(**폴백 전용** — 09-03부터 선택) |
|---|---|---|---|
| **Debian · Ubuntu · Mint · Pop!\_OS** | `sudo apt install build-essential` | `sudo apt install libxkbcommon-x11-0` | `sudo apt install wl-clipboard xclip` |
| **Fedora · RHEL · Rocky · Alma** | `sudo dnf install gcc` (또는 `sudo dnf group install development-tools`) | `sudo dnf install libxkbcommon-x11` | `sudo dnf install wl-clipboard xclip` |
| **Arch · Manjaro · EndeavourOS** | `sudo pacman -S base-devel` | `sudo pacman -S libxkbcommon-x11` | `sudo pacman -S wl-clipboard xclip` |
| **openSUSE** | `sudo zypper install gcc` (또는 패턴 `devel_basis`) | `sudo zypper install libxkbcommon-x11-0` | `sudo zypper install wl-clipboard xclip` |
| **Alpine** | `doas apk add build-base` | `doas apk add libxkbcommon` | `doas apk add wl-clipboard xclip` |
| **NixOS** | `pkgs.gcc` | `pkgs.libxkbcommon` | `pkgs.wl-clipboard` · `pkgs.xclip` |

Rust 툴체인은 **어느 배포판에서도 `rustup`을 쓴다** — 배포판 패키지 `rustc`는 쓰지 않는다.

- `rust-toolchain.toml`이 stable + `rustfmt`/`clippy` + **4개 크로스 타깃**을 고정하는데,
  배포판 `rustc`는 rustup이 아니라 이걸 못 따른다.
- 버전도 자주 뒤진다 — 우리 최소는 `rust-version = "1.82"`다. LTS 계열(Debian stable ·
  Ubuntu 22.04 · RHEL)은 이보다 낮은 `rustc`를 담고 있는 일이 흔하다.
  *(실측 08-29 — Ubuntu 26.04는 1.93으로 충분했지만, 그건 그 배포판이 새것이라서다.)*

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 9-3. ★ sudo를 쓸 수 없을 때 — 루트 없는 로컬 프리픽스

공용 개발기·권한 제한 계정에서 시스템을 건드리지 않고 갖추는 방법이다(08-29 실사용).
`apt-get download`는 **root가 필요 없다**.

```bash
PREFIX=$HOME/.local/nclip-devenv
mkdir -p "$PREFIX/debs" "$PREFIX/root" && cd "$PREFIX/debs"

# 의존 폐포에서 '아직 없는 것'만 추린다
apt-cache depends --recurse --no-recommends --no-suggests --no-conflicts \
  --no-breaks --no-replaces --no-enhances gcc wl-clipboard xclip \
  | grep '^[a-z]' | sort -u > all.txt
: > need.txt
while read -r p; do dpkg -s "$p" >/dev/null 2>&1 || echo "$p" >> need.txt; done < all.txt

apt-get download $(tr '\n' ' ' < need.txt)
for d in *.deb; do dpkg-deb -x "$d" "$PREFIX/root"; done

# rustc 는 `cc` 라는 이름을 찾는다
ln -sf x86_64-linux-gnu-gcc-15 "$PREFIX/root/usr/bin/cc"
```

⚠️ **함정 — `cc1`을 못 찾는다.** gcc 드라이버는 보조 실행 파일(`cc1`)을 **자기 위치 기준**으로
찾는다. `cpp-N`이 시스템에 이미 깔려 있으면 gcc 본체만 로컬로 풀리므로 경로가 어긋난다.
시스템 것을 로컬 프리픽스에 이어 준다:

```bash
ln -sf /usr/libexec/gcc/x86_64-linux-gnu/15/cc1 \
       "$PREFIX/root/usr/libexec/gcc/x86_64-linux-gnu/15/cc1"
```

되돌리기는 `rm -rf ~/.local/nclip-devenv` 하나다 — **시스템은 건드리지 않았다**.

> ⚠️ **에디터를 root로 실행하지 말 것.** VSCode 등을 `sudo`로 띄우면 그 안에서 도는 모든
> 명령이 root가 되어 `target/` · `~/.cargo` · 저장소 파일이 root 소유로 바뀐다. 이후 평소
> 계정으로 `git`·`cargo`를 쓸 때 권한 오류가 줄줄이 나고 `chown`으로 되돌려야 한다.

### 9-4. 표시 서버 — Wayland vs X11

`XDG_SESSION_TYPE`이 `wayland`인지 `x11`인지가 백엔드를 가른다([`watch_linux`](../crates/nclip-plat/src/watch_linux.rs)).

```bash
echo "$XDG_SESSION_TYPE / $XDG_CURRENT_DESKTOP"
```

- **Wayland 세션에도 `DISPLAY`가 같이 있다**(XWayland). 그래서 백엔드 선택은 `WAYLAND_DISPLAY`를
  **먼저** 본다 — `DISPLAY`가 있다고 X11로 가면 XWayland 앱의 클립보드만 보게 된다.
- 요즘 배포판 기본값은 대체로 Wayland다(Ubuntu 22.04+ GNOME · Fedora GNOME · Debian 12 GNOME).
  X11 세션은 로그인 화면에서 따로 골라야 하는 일이 많고, Fedora처럼 **X11 세션을 아예 빼는**
  방향으로 가는 배포판도 있다.

### 9-5. ★ 컴포지터 차이 — 여기가 Linux 클립보드의 핵심

Wayland는 보안 모델상 **아무 앱이나 클립보드를 훔쳐볼 수 없다**. 클립보드 매니저는
별도 프로토콜이 있어야 하는데, **컴포지터마다 지원이 다르다**.

| 컴포지터 | `wlr-data-control` | 우리에게 미치는 영향 |
|---|:--:|---|
| **wlroots 계열**(Sway · Hyprland · river · Wayfire) | ✅ | 본편(T-14 본체) 이벤트 구독 가능 |
| **KWin**(KDE Plasma) | ✅ | 동일 |
| ★ **Mutter**(GNOME) | ❌ | ★ **이것 때문에 1단이 도구 파이프다** — ✅ **실측 08-29**: Mutter **50.1**(Ubuntu 26.04) `wayland-info` = `wl_data_device_manager v3`뿐 · `zwlr`·`ext` data-control **둘 다 없음** |
| **Weston** | ❌ | ✅ **실측 08-29**: **14.0.2** nested — data-control 없음. `wl-paste`는 seat만 있으면 동작(헤드리스 백엔드는 seat가 없어 **wl-clipboard가 거부**) |

> ★ **GNOME이 이 설계를 결정했다.** 데스크톱 점유율 1위가 `data-control`을 제공하지 않으므로,
> 프로토콜 구독만 구현하면 **가장 많은 사용자에게서 동작하지 않는다**. 그래서 1단은
> `wl-paste` 파이프 + 폴링이다 — `wl-clipboard`는 프로토콜이 없으면 **숨은 표면(surface)** 을
> 만들어 일반 클립보드 경로로 읽기 때문에 GNOME에서도 동작한다.
>
> ✅ **실측 08-29** — Ubuntu 26.04 · GNOME/Wayland에서 `wayland-wl-paste` 백엔드로 8유형 전부 포착.

⚠️ **확인 필요** — 표준 후속 프로토콜 `ext-data-control-v1`이 최근 채택되고 있다.
어느 컴포지터가 어느 버전부터 싣는지는 **실기로 확인한 뒤** 이 표를 갱신한다(D-75 운영 방식).
확인 명령은 `wayland-info | grep data_control`(패키지 `wayland-utils` — 루트 없이도 §9-3 방식으로 놓인다).
KWin·wlroots 계열은 이 PC에 없어 **아직 문서 기반**이다.

✅ **X11 경로 실측 08-29** — `x11-xclip` 백엔드를 **XWayland**(`WAYLAND_DISPLAY`만 지우면 Mutter가
X 클립보드를 양방향으로 잇는다)와 **순수 `Xvfb`** 에서 7/7 통과([21 §2-7](21-manual-test.md)).
X11 세션 로그인 없이도 두 방법으로 X11 경로를 검증할 수 있다.

### 9-6. ★ 파일 관리자 — 잘라내기/복사 표현이 갈린다

Linux에는 "복사한 파일"의 **표준 표현이 하나가 아니다**. 잘라내기/복사 구분은
`text/uri-list`가 아니라 **별도 표현**에 실린다.

| 파일 관리자 | 데스크톱 | 표현 | 잘라내기 표시 |
|---|---|---|---|
| **Nautilus** | GNOME | ✅ **실측 08-29**(GNOME 50 · 사용자 `Ctrl+C`/`Ctrl+X`): `text/uri-list` + `text/plain;charset=utf-8` + `x-special/gnome-copied-files` + ★ **`application/vnd.portal.files`·`application/vnd.portal.filetransfer`**(xdg-desktop-portal 전송 키 — 곁다리 처리) | 본문 첫 줄 `cut` / `copy` (실측: 92B→91B) |
| **Dolphin** | KDE | `text/uri-list` + `application/x-kde-cutselection` | 표식 값 `1` |
| Thunar · Nemo · Caja · PCManFM | XFCE · Cinnamon · MATE · LXDE | gnome 이름을 함께 쓰는 것으로 알려져 있다 | 동일 |

- ✅ **처리함**(08-29) — `x-special/gnome-copied-files` · `x-special/KDE-copied-files` ·
  `x-special/nautilus-clipboard`를 [`is_files_format`](../crates/nclip-core/src/capture.rs)에,
  `application/x-kde-cutselection`을 곁다리(`is_metadata_format`)에 넣었다.
  첫 줄 `cut`/`copy`는 `parse_uri_list`가 `file://` 줄만 받으므로 저절로 걸러진다.
- ⚠️ **경로 중복** — GNOME은 `text/uri-list`와 `x-special/gnome-copied-files`를 **함께** 내놓는다.
  `file_paths()`가 중복을 제거하지 않으면 **파일 하나가 목록에 둘로 보인다**.
- ⏳ **실기 확인 대상** — 위 표에서 **Nautilus는 실측**, 그 외는 **문서·관례 기반**이다. Dolphin·Thunar가
  실제로 무엇을 내놓는지는 그 데스크톱에서 `wl-paste --list-types`로 확인하고 표를 갱신한다.

### 9-7. 민감 표식 — Linux에는 사실상 KDE 관례뿐

- `x-kde-passwordManagerHint` 값이 `secret`이면 기록하지 않는다(KDE/Klipper 관례).
  KeePassXC 등이 이걸 붙인다. **값을 못 읽으면 금지로 본다**(fail-closed · FR-S-1).
- ⚠️ **브라우저 암호 관리자는 이 표식을 붙이지 않는다** — Windows·macOS와 같은 한계다.
  옵트인 `sec.conceal_browser_pw`(D-79)가 그 자리를 메운다.
- macOS `org.nspasteboard.*` 계열에 해당하는 **범용 Linux 관례는 없다**.

### 9-8. 클립보드 매니저 충돌

Linux에서 클립보드 내용은 **원본 앱이 소유**한다 — 앱이 닫히면 내용이 사라진다.
그래서 대부분의 데스크톱이 자체 클립보드 매니저를 함께 돌린다(KDE Klipper · GNOME GPaste 확장 등).

⚠️ 다른 매니저가 떠 있으면 **셀렉션 주인이 자주 바뀐다** — 읽기가 순간 실패할 수 있다.
폴링 루프는 이걸 실패로 보지 않고 **다음 틱에 다시 읽는다**(그리고 유휴로 물러난다 — [§9-9](#9-9-상주-예산)).

### 9-9. 상주 예산

Windows(`GetClipboardSequenceNumber`)·macOS(`changeCount`)와 달리 **Linux에는 싼 변경 신호가 없다**.
1단은 틱마다 내용을 읽어 **지문(FNV)** 으로 비교하므로, 틱 하나가 곧 프로세스 생성이다.

- 주기: 활동 **500ms** → 유휴 **2s**(macOS 200ms→1s보다 느슨한 이유가 이것이다).
- ★ **못 읽음 · 빈 스냅숏 · 같은 지문은 전부 "변화 없음"** 이라 유휴로 물러난다.
  ⚠️ 08-29 결함 — 빈 클립보드에서 카운터를 0으로 되돌리던 탓에 **영구히 활동 주기로 돌았다**
  (`wl-paste --list-types`는 빈 클립보드에서 비정상 종료한다 → 매 틱 실패 → 매 틱 리셋).
  로그인 직후처럼 오래 가는 정상 상태였다(DR-9 위반).
- 읽기 상한 64 MB는 **읽으면서** 건다 — 다 읽고 버리면 그 순간 메모리는 이미 먹었다.

### 9-9-1. ★ 부분 스냅숏 — 폴링만의 함정

일련번호가 없는 폴링은 **앱이 표현을 다 올리기 전에** 읽을 수 있다.

⚠️ 08-29 실기 — `text/html` 복사가 `text/plain` 하나만 보이는 순간에 잡혀
**같은 복사가 두 항목**이 됐다. 먼저 것은 표현이 모자라 `Text`로 **오분류**됐다.
반대 방향도 나왔다 — 앞 복사의 표현이 잠깐 남아
`{text/plain, gnome-copied-files}` → `{gnome-copied-files}` 로 **줄어드는** 전이.

- 수신 루프의 디바운스(D-80 · 500ms)가 원래 이걸 합치는 자리인데, **Linux 폴링 주기가
  500ms라 완본이 창 밖에 떨어진다** → 백엔드가 먼저 다잡는다(`settle`).
- 규칙은 하나 — **두 번 같게 읽힐 때까지** 기다리고(120ms × 최대 4회) 늦게 읽힌 것이 정본이다.
  `coalesces`(부분 ⊆ 완본)를 쓰지 않는 이유는 **줄어드는 전이를 못 걸러서**다.
- ★ **480ms 안의 두 번째 복사를 잃지 않는다** — 폴링 주기가 이미 500ms라 그보다 짧게
  스쳐 간 복사는 애초에 보이지 않는다. 자리 잡기는 폴링이 줄 수 있는 것을 깎지 않는다.

### 9-10. 이 환경에서 확인하는 법

```bash
cargo run -p nexa-clip -- peek     # 지금 클립보드 한 번 — 백엔드 이름이 찍힌다
cargo run -p nexa-clip -- watch    # 계속 감시. NEXA_CLIP_DIAG=1 로 진단 켬
```

`watch`가 거부하면 **사유마다 조치가 함께 찍힌다**(`MissingTool`이면 배포판별 설치 명령).
조용히 빈 목록을 돌려주는 일은 없다([docs/02](02-roadmap.md) R-4).

유형별 자동 실기는 [21 실기 점검표](21-manual-test.md)의 Linux 절차를 따른다.

### 9-11. ★ 트레이 · 전역 단축키 · 키 주입 — 데스크톱 셸 의존(08-30 실측)

| 기능 | 통로 | 이 PC(Ubuntu 26.04 · GNOME 50 · Wayland) | 없을 때 앱이 하는 말 |
|---|---|---|---|
| **트레이** | SNI(`org.kde.StatusNotifierWatcher`) | ✅ `ubuntu-appindicators@ubuntu.com` 확장이 워처(`busctl --user list \| grep StatusNotifier`) | `트레이를 띄울 수 없습니다 — 세션 버스에 StatusNotifierWatcher가 없음 …` — GNOME은 AppIndicator 확장, KDE/Sway(waybar)는 내장 |
| **전역 단축키** | xdg 포털 `org.freedesktop.portal.GlobalShortcuts` v1 | ✅ (`busctl --user introspect org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.GlobalShortcuts`) · 첫 등록 = **사용자 확인 대화창** | `등록 실패 — xdg 포털 GlobalShortcuts가 없거나(GNOME 48+ · KDE 6+) 거부됨` → 트레이 좌클릭 |
| **키 주입** | Wayland = ★ **xdg 포털 `RemoteDesktop`**(첫 회 "원격 제어" 승인 · `restore_token` 영구) · X11 = `XTest`(x11rb) | ✅ `nexa-clip` → `paste inject : ok (wayland-portal-remotedesktop)`. ⚠️ Xwayland `-enable-ei-portal`에선 **XTest가 앱까지 못 간다**(08-30 실기 정정) | 포털도 `DISPLAY`도 없음 = `WaylandNoInjection` — 클립보드 적재까지만(FR-P-1) |
| **클립보드 읽기/쓰기** | ★ **data-control 있는 컴포지터만 `wl-paste`/`wl-copy`** · 없으면(GNOME) **XWayland `xclip`**(Mutter 양방향 동기화) — 판정은 `wayland_probe`(레지스트리 사실) | ✅ `clipboard watch : ok (xwayland-xclip)`. ⚠️ **08-30 실기**: data-control 없는 곳에서 wl-clipboard는 **매 호출 숨은 창으로 포커스를 뺏는다** — 폴링이면 타이핑 불가 | `클립보드 쓰기 도구가 없습니다 — xclip` |
| **창 앞으로(Wayland)** | 셸 발급 `xdg_activation` 토큰(SNI `ProvideXdgActivationToken`) | appindicator 확장이 준다 | 토큰 없음 = "앱이 준비되었습니다" 알림(Dock 강조) → 클릭 복귀 |

자동 확인: K-1 X11 경로는 `Xvfb :99 -screen 0 640x480x24 & DISPLAY=:99 cargo test -p nclip-plat -- --ignored x11_xtest`
(WAYLAND_DISPLAY는 비운다). 트레이는 D-Bus로 사람 없이 재현 가능 — `busctl --user call <이름> /MenuBar
com.canonical.dbusmenu GetLayout iias -- 0 -1 0` · `Event isvu -- <id> clicked s "" 0`(id 3 열기 · 4 종료 · 100+ 최근).
⚠️ `pkill -f "nexa-clip tray"`는 그 문자열을 인자로 가진 셸 자신도 죽인다 — `pgrep -x nexa-clip`으로 확인.

---

## 10. 배포 — `release.yml` · brew · winget · Chocolatey (09-04)

> 포장 상세(채널·타깃·매니페스트·스위치·검수 대기 판정)는 [`packaging/README.md`](../packaging/README.md)가 SSOT.
> 이식 원본 = `nexa-beep`(08-11 정책) + ★ **검수 대기 자동 판정**(09-04 사용자 요청).

```bash
# 1) main이 green인지 확인한다 — red 상태로 태그를 밀지 않는다.
gh run list --branch main --workflow ci --limit 1
# 2) 버전을 올린다(워크스페이스 단일 버전) — 태그와 다르면 meta 잡이 멈춘다.
$EDITOR Cargo.toml       # [workspace.package] version
# 3) 태그를 밀면 끝 — 5타깃 빌드 · 릴리스 공개 · brew 탭 · winget/choco 제출이 이어진다.
git tag v0.1.0 && git push origin v0.1.0
```

- winget·choco는 저장소 변수 `WINGET_PUBLISH`·`CHOCO_PUSH`(= true · 09-04 설정)와 시크릿(`WINGET_TOKEN`·`CHOCO_API_KEY`·`TAP_TOKEN` · 09-04 사용자 추가)이 있어야 나간다.
- ★ **직전 제출이 검수 대기 중이면 그 채널은 자동으로 건너뛴다**(guard 잡 — winget 열린 PR · choco 피드 부재). 릴리스·brew·다른 채널은 그대로.
  사람이 확인한 뒤 강제하려면 `publish-windows-packages` 수동 실행 `force=true`.
- 태그 없이 산출물만 보려면 Actions → release → *Run workflow*(초안).
