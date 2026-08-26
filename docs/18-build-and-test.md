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
| 외부 도구 | **없음** | 빌드에 C 툴체인·시스템 라이브러리가 필요 없다 |

```bash
git clone git@github.com:SosomLab/nexa-clip.git
cd nexa-clip
cargo build          # 첫 빌드에서 툴체인·의존을 받는다
```

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
