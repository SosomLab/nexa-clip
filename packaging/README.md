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
| `linux-x64` | `.deb` | `.tar.gz` |

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
| Chocolatey | **직전 정식 태그 버전**이 공개 피드에 아직 없다 | `api/v2/Packages()?$filter=Id eq '<pkg>' and Version eq '<prev>'` — 모더레이션 중 패키지는 피드에 숨는다 |

대기면 그 채널만 건너뛰고(**릴리스·brew·다른 채널은 그대로 나간다**) 경고로 이유를 남긴다. 사람이 확인한 뒤 강제로 내려면
Actions → publish-windows-packages → *Run workflow* · `force=true`. 첫 제출(직전 태그 없음)은 판정 없이 나간다.

변수가 꺼져 있거나 판정에 걸려도 매니페스트는 **항상 만들어** 아티팩트·릴리스 자산으로 올린다 — 손으로 제출할 수 있게.

## ⚠️ macOS 격리(quarantine)

서명·공증이 없는 앱은 격리 표식이 붙어 있으면 실행 즉시 SIGKILL 된다(beep 08-11 실측 · 애드혹 서명으로도 못 넘음).
→ `.app`에 애드혹 서명 + **Cask `postflight`에서 격리 표식 제거**(caveats에 그대로 밝힌다). `.dmg`를 직접 받은 경우:

```bash
xattr -dr com.apple.quarantine "/Applications/Nexa Clip.app"
```

## 설치 위치와 권한

Windows 설치본은 사용자 단위(`%LOCALAPPDATA%\Programs\NexaClip` · HKCU) — 관리자 권한 불요, winget/choco 무인 설치(`/S`) 통과.
설치본 실행·바로가기는 **인자 없이**(= 트레이 상주 · 콘솔 없음). 데이터는 실행 파일 옆 `data/`(포터블) 또는 사용자 설정 폴더.

## 아직 아닌 것

- **서명하지 않는다**(v1) — 인증서가 없다. 별도 결정으로 다룬다.
- **Linux는 x86_64만**. arm64는 수요 확인 후.
