# packaging — 배포 산출물 만들기

> 워크플로 = [`.github/workflows/release.yml`](../.github/workflows/release.yml) · [`homebrew.yml`](../.github/workflows/homebrew.yml) ·
> [`publish-windows-packages.yml`](../.github/workflows/publish-windows-packages.yml).
> 이식 원본 = `nexa-beep/packaging`(08-11 사용자 확정 정책 승계) + ★ **검수 대기 자동 판정**(09-04 사용자 요청).
> 개발자 절차는 [docs/18 §10](../docs/18-build-and-test.md#10-배포).

## 채널과 타깃

**설치본 + 포터블 2채널**을 **5개 타깃**에 낸다.

| 타깃 | 설치본 | 포터블 |
| --- | --- | --- |
| `windows-x64` | NSIS `.exe` (+`.zip`) | `.zip` |
| `windows-arm64` | NSIS `.exe` (+`.zip`) | `.zip` |
| `macos-arm64` | `.dmg` | `.tar.gz` |
| `macos-x64` | `.dmg` | `.tar.gz` |
| `linux-x64` | `.deb` · `.rpm` | `.tar.gz` |

압축 형식은 플랫폼 관례(Windows zip · mac/Linux tar.gz = 실행 권한 보존). `setup.exe`는 zip 사본을 하나 더 올린다
(실행 파일 확장자를 막는 브라우저·사내 프록시). `SHA256SUMS.txt`를 함께 올린다 — 서명이 없는 배포에서 유일한 검증 수단.
두 실행 파일(`nexa-clip` + `nclip-imgdec` 이미지 격리 디코드 워커)을 항상 함께 담는다.

## 패키지 관리자 — 세 채널 모두 포함

| 채널 | 이름 | 설치 |
| --- | --- | --- |
| Homebrew Cask(macOS 설치본) | `nexa-clip` | `brew install --cask kiros33/tap/nexa-clip` |
| Homebrew Formula(macOS/Linux 포터블) | `nexa-clip-portable` | `brew install kiros33/tap/nexa-clip-portable` |
| winget 설치본 / 포터블 | `SosomLab.NexaClip` / `SosomLab.NexaClip.Portable` | `winget install SosomLab.NexaClip[.Portable]` |
| Chocolatey 설치본 / 포터블 | `nexa-clip` / `nexa-clip-portable` | `choco install nexa-clip[-portable]` |

매니페스트 **틀**은 이 디렉터리(`winget/`, `choco/`, `homebrew/`)에 있고, **`render-manifests.sh`가 유일한 치환 지점**이다
— 실제 산출물에서 SHA256을 계산해 채우고, 자리표시자가 하나라도 남으면 멈춘다. 릴리스에 `...package-manifests.zip`으로 첨부한다.

## 트리거와 스위치

| 무엇 | 언제 | 잠금 |
| --- | --- | --- |
| GitHub Release(설치본·포터블·체크섬·매니페스트) | `v*` 태그 push → 자동 공개 | 없음 |
| Homebrew 탭(macOS/Linux) | 릴리스 직후 자동 | `TAP_TOKEN` 시크릿 유무 |
| winget · Chocolatey(Windows) | 릴리스 직후 자동 | 변수 `WINGET_PUBLISH`/`CHOCO_PUSH`=true + 시크릿 `WINGET_TOKEN`/`CHOCO_API_KEY` + ★ **검수 대기 판정** |

```bash
git tag v0.1.0 && git push origin v0.1.0     # 이게 전부 — 태그 = Cargo.toml 버전이어야 한다(meta 잡이 검사)
```

### ★ 검수 대기 자동 판정 (09-04)

winget·Chocolatey는 중앙 검수를 거치며, **직전 제출이 검수 통과 전이면 새 버전을 제출하지 않는다**(검수 중 새 제출은 큐를
엉키게 하고 반려 사유가 된다 — beep 08-24 규칙). beep에서는 사람이 `gh pr view`·choco 피드로 점검해 스위치를 켜고 껐지만,
여기서는 `publish-windows-packages.yml`의 **guard 잡**이 릴리스마다 스스로 판정한다.

| 채널 | 대기로 보는 조건 | 판정 근거 |
| --- | --- | --- |
| winget | microsoft/winget-pkgs에 토큰 주인이 낸 **열린 PR**이 `SosomLab.NexaClip`를 달고 있다 | `gh pr list --state open --author <me> --search SosomLab.NexaClip` |
| Chocolatey | **패키지별**: 그 패키지가 choco에 마지막으로 낸 버전이 아직 승인되지 않았다(10-05 수정 — 단계 이름 "마지막 제출 버전이 승인됐는가" · ★ 10-10 패키지별 판정 `c7fb80d` — 승인된 패키지만 guard 출력 `pkgs`로 push · 포터블 0.1.5 승인·설치본 0.1.5 검수 중일 때 둘 다 skip하던 묶음 판정 폐기) | 태그를 최신부터 훑어 `api/v2/Packages()?$filter=Id eq '<pkg>' and Version eq '<v>'`에 `<entry>`가 처음 나오는 버전을 찾고, 그 응답에 `<d:IsApproved m:type="Edm.Boolean">true`가 있는가 — ⚠️ 이 Id+Version 조회는 **검수 중(미승인) 패키지도 항목을 돌려준다**(`IsApproved=false` · `PackageStatus=Submitted`) · Id만으로 조회하면 미승인은 안 나온다(그래서 버전을 하나씩 묻는다). 종전(10-04)의 "직전 git 태그 버전" 기준은 그 버전을 choco에 못 낸 경우(0.1.6 — 403) 항목이 없어 **이후 계속 건너뛰는** 결함이 있었다 · 조회 실패 = 건너뜀 |

> ⚠️ **10-04 결함과 수정** — 종전 판정은 `<entry>` 유무("모더레이션 중 패키지는 피드에 숨는다"는 전제)였는데, 그 전제가 **틀렸다**: Id+Version 조회는 검수 중 0.1.5도 돌려줘 guard가 "공개됨"으로 오판 → v0.1.6 push → Chocolatey **403** → chocolatey 잡 빨강(0.1.6은 제출 안 됨). 수정 = `IsApproved=true`로 판정 · 대조: clip 0.1.5 두 패키지 = 미승인(skip) · nexa-beep 0.2.2 = 승인(go · 양성). **교훈 — 게이트의 전제를 실측 없이 믿지 않는다 · 가드도 음성·양성 대조를 돌린다.**

대기면 그 채널만 건너뛰고(**릴리스·brew·다른 채널은 그대로 나간다**) 경고로 이유를 남긴다. 사람이 확인한 뒤 강제로 내려면
Actions → publish-windows-packages → *Run workflow* · `force=true`. 첫 제출(어느 태그 버전도 피드에 없음)은 판정 없이 나간다.

변수가 꺼져 있거나 판정에 걸려도 매니페스트는 **항상 만들어** 아티팩트·릴리스 자산으로 올린다 — 손으로 제출할 수 있게.

### 🔴 사람 검수 — 점검법과 메타데이터 규칙 (10-04)

자동 검사(Chocolatey validator·verifier · winget 파이프라인)를 **통과한 뒤에도 사람 검수자가 돌려보낼 수 있다.** 0.1.5 choco는
Validation·Verification Passed 뒤 검수자가 *"설명이 한국어뿐 · 비상업 제한이 영어로 안 읽힘 · `<copyright>` 없음 — 고쳐서 같은 버전으로
다시 내라"*(09-28)고 댓글을 달았는데, 배지만 보고 "모더레이터 대기"로 오판해 6일을 허비했다([journal 10-04](../docs/journal/2026-10-04.md)).

| 규칙 | 내용 |
| --- | --- |
| **A. 점검** | choco = 패키지 페이지 `https://community.chocolatey.org/packages/<pkg>/<ver>`의 **상태 문구 + 댓글 전문**(피드 API는 미승인 패키지를 숨긴다 · 배지는 자동 검사 결과뿐). ★ **"Waiting for Maintainer" = 우리 차례.** winget = 라벨 + **PR 댓글**. |
| **B. 전부 영어** | 🔴 **winget·Chocolatey 제출 틀(`winget/**` · `choco/**`)은 전부 영어**(사용자 10-04) — 설명·요약 · 스크립트 주석 · `Write-Host` 문구 · YAML 주석까지 · **한국어 병기 없음** · 기본 로캘 `en-US` · 비상업 제한은 영어 문장(`License: … noncommercial use …`) · `<copyright>` 필수. 이 디렉터리의 다른 파일(이 README · `homebrew/` — 우리 탭 저장소)은 대상 아님. 강제 = `render-manifests.sh` 게이트 — ① `<copyright>` ② 영어 License 줄 ③ winget·choco 출력 전체 ASCII 밖 글자 0(ps1 머리 BOM만 예외) · 어기면 `exit 1`. 게이트를 고치면 음성·양성 대조를 둘 다 돌린다. |
| **C. 본보기** | 새 채널·새 패키지 메타데이터는 **beep 것을 먼저 대조**한다(이미 통과한 본보기). |
| **D. 재제출** | 반려 뒤에는 **같은 버전**으로. 제출 워크플로는 **기본 브랜치의 `packaging/`** 을 쓰므로(자산만 태그에서) 수정을 main에 push한 **뒤** `publish-windows-packages` `tag=vX.Y.Z force=true` · 그 실행 동안 `WINGET_PUBLISH=false`(force가 winget guard도 무시) → 끝나면 복구. |

⚠️ winget 틀은 10-04에 `en-US`로 바꿨지만, 이미 열린 PR #434300·#434302(0.1.4)는 `ko-KR`·한국어 그대로다 — 교체 여부는 사용자 결정 대기([TODO T-50](../docs/TODO.md)).

## ⚠️ Windows — 정적 CRT (09-14)

Rust `*-pc-windows-msvc`의 기본값은 **CRT 동적 링크**라 산출물이 `VCRUNTIME140.dll`을 요구한다. 이 DLL은 Windows 구성요소가
아니라 **VC++ 재배포 패키지**로만 깔린다 — 즉 기본값 그대로 배포하면 **깨끗한 Windows에서 실행 즉시 죽는다**
(`0xC0000135 STATUS_DLL_NOT_FOUND` · winget 검수기가 실측해 v0.1.3 제출이 반려됐다: PR #431182/#431183).
→ [`.cargo/config.toml`](../.cargo/config.toml)에서 두 windows-msvc 타깃에 `-C target-feature=+crt-static`.

🔴 **워크플로에 env `RUSTFLAGS`를 넣지 말 것** — env `RUSTFLAGS`는 `.cargo/config.toml`의 target별 `rustflags`를 **통째로
덮어써** 정적 CRT가 **릴리스 산출물에서만 조용히 빠진다**(로컬은 멀쩡하므로 안 드러난다). `release.yml`의 env는 그래서 걷어냈고,
경고 게이트는 `ci.yml`(clippy `-D warnings`)·`scripts/check-3os.sh`가 맡는다. 마지막 방어선은 `release.yml`의
**Windows DLL 게이트** — 산출물에 `vcruntime*`/`msvcp*`/`msvcr*` 임포트가 보이면 배포를 멈춘다(Linux glibc 심볼 게이트의 짝).

## ⚠️ macOS 격리(quarantine)

서명·공증이 없는 앱은 격리 표식이 붙어 있으면 실행 즉시 SIGKILL 된다(beep 08-11 실측 · 애드혹 서명으로도 못 넘음).
→ `.app`에 애드혹 서명 + **Cask `postflight`에서 격리 표식 제거**(caveats에 그대로 밝힌다). `.dmg`를 직접 받은 경우:

```bash
xattr -dr com.apple.quarantine "/Applications/Nexa Clip.app"
```

## ⚠️ macOS 손쉬운 사용 권한과 업그레이드

TCC(권한 DB)는 앱을 번들 ID + **코드 서명 요구사항**으로 기억한다. 애드혹 서명은 요구사항이 바이너리 해시(`cdhash`)라 **업그레이드마다 옛 항목이 새 앱과 맞지 않는다** — 토글이 ON으로 보여도 `AXIsProcessTrusted = false`, 껐다 켜도 무효, 항목 삭제만 통한다(09-07 사용자 실기).
→ Cask `postflight`와 앱 시작(`nclip-plat paste::warm_up`) 양쪽에서 `tccutil reset Accessibility io.github.sosomlab.nexa-clip`(sudo 불요)을 돌린 뒤 권한 대화상자를 띄운다 — 사용자는 [켜기]만. 근본 처방은 안정 서명 신원(요구사항이 인증서가 된다 · [TODO T-48](../docs/TODO.md)).

## 설치 위치와 권한

Windows 설치본은 사용자 단위(`%LOCALAPPDATA%\Programs\NexaClip` · HKCU) — 관리자 권한 불요, winget/choco 무인 설치(`/S`) 통과.
설치본 실행·바로가기는 **인자 없이**(= 트레이 상주 · 콘솔 없음). 데이터는 실행 파일 옆 `data/`(포터블) 또는 사용자 설정 폴더.

## 아직 아닌 것

- **서명하지 않는다**(v1) — 인증서가 없다. 별도 결정으로 다룬다.
- **Linux는 x86_64만**. arm64는 수요 확인 후.

## Linux — 배포판 비종속(09-04)

beep 서버는 musl 정적이지만 GUI 앱은 winit·xkbcommon·Wayland를 **런타임 dlopen**하므로 musl 정적이 불가하다.
같은 목표(어느 배포판·glibc 버전에서든 바로 실행)를 **zig 링커로 glibc 2.17 기준 링크**해 얻는다:

- `cargo zigbuild --target x86_64-unknown-linux-gnu.2.17` — 산출물이 요구하는 glibc 심볼 상한 = 2.17(CentOS 7·Ubuntu 14.04 이후 전부).
- Cargo features `wayland-dlopen`(winit·softbuffer) + `wayland-backend/dlopen`(nclip-plat) — 시스템 라이브러리를 **빌드 시 링크하지 않는다**.
- 워크플로 **심볼 게이트**: `NEEDED`에 glibc 계열 외 라이브러리가 있거나 `GLIBC_` 최고 버전이 2.17을 넘으면 배포를 멈춘다.
- `.deb` Depends = `libc6 (>= 2.17)`. 런타임 필요 라이브러리(libxkbcommon · libX11/libwayland 중 쓰는 쪽)는 없으면 그 경로만 빠진다.
- `.rpm`(10-05)은 `.deb`와 같은 내용물을 `packaging/linux/nexa-clip.spec`으로 포장만 달리한다(러너의 `rpmbuild`). 의존은 rpmbuild가 실행 파일에서 뽑은 것(glibc 계열)이고, 패키지 서명은 없다 — `SosomLab/linux-repo`가 이 자산으로 pkg.sosomlab.com의 dnf 저장소를 만들며 무결성은 저장소 메타데이터 서명이 지킨다.
