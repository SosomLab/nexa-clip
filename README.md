# Nexa Clip

**3-OS(Windows · macOS · Linux)에서 똑같이 생긴 화면으로 동작하는 클립보드 매니저.**
전부 Rust · 단일 실행 파일 · 자체 CPU 래스터라이저(Qt·WebView·Electron 없음).

- 이력은 **로컬에 암호화 저장**(기본 켜짐), 기기 사이 전송은 **Noise E2E**로 봉인(서버는 봉투만 봄).
- 동기화는 **선택**: 같은 네트워크 직결(서버 불요) 또는 릴레이 경유 · **승인한 기기**와만 공유.
- 상주 예산을 지킨다(유휴 RSS 수십 MB · 기동 수백 ms 목표).

## 설치

| | 설치본 | 포터블 |
| --- | --- | --- |
| Windows | `winget install SosomLab.NexaClip` · `choco install nexa-clip` | `winget install SosomLab.NexaClip.Portable` · `choco install nexa-clip-portable` |
| macOS | `brew install --cask kiros33/tap/nexa-clip` | `brew install kiros33/tap/nexa-clip-portable` |
| Linux | `.deb`(Releases) | `brew install kiros33/tap/nexa-clip-portable` · `.tar.gz` |

직접 받기: <https://github.com/SosomLab/nexa-clip/releases> — `SHA256SUMS.txt`로 무결성을 확인할 수 있다.
⚠️ v1은 **코드 서명이 없다** — macOS Gatekeeper·Windows SmartScreen 경고가 뜬다(brew Cask는 격리 표식을 설치 시 뗀다).

## 사용

- 실행하면 트레이에 상주한다. 좌클릭 = 메인창(항목 관리) · 우클릭 = 최근 메뉴 · `Ctrl+Shift+V` = 퀵 팝업.
- 사용자 안내는 **[위키](https://github.com/SosomLab/nexa-clip/wiki)**, 설계·진행 기록은 [`docs/`](docs/)에 있다.

## 라이선스

[PolyForm Noncommercial 1.0.0](LICENSE.md) — 비상업적 사용. ([한국어 번역](LICENSE.ko.md))
