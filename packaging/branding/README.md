# Nexa Clip 브랜딩 자산

앱 아이콘의 **SSOT**. SosomLab "Nexa" 계열(풀블리드 라운드 스퀘어)을 계승하되, Nexa Clip의
정체성인 **클립보드 기록**에 맞춰 **클립보드(집게 달린 보드) + 뒤로 겹친 카드 2장(히스토리)**
모티프로 그렸다.

## 계열 안에서의 자리

| 앱 | 모티프 | 색 |
| --- | --- | --- |
| **nexa-dir2** | 폴더 + `>_`(터미널) | 다크 네이비 + 파란 테두리 + **초록** 액센트 |
| **nexa-beep** | 말풍선 + 비콘 파동 | **파랑** `#4A97FF → #2C6BE6` |
| ★ **nexa-clip** | **클립보드 + 히스토리 스택** | **청록** `#22C3D6 → #0B7FA6` |

> 계열 공통 = 풀블리드 라운드 스퀘어(반경 232/1024) · 세로 그라디언트 배경 · 흰 전경 도형 ·
> accent 단색 모티프. **색과 모티프만 앱마다 다르다** — 작업 표시줄에서 한눈에 구분된다.

## 파일

| 파일 | 크기 | 용도 |
| --- | --- | --- |
| `icon.svg` | 벡터 | **원본 SSOT** — 아이콘 변경은 여기부터 |
| `nexa-clip-1024.png` | 1024 | 스토어·고해상도(macOS `.app`/`.icns` 원본) |
| `nexa-clip-256.png` | 256 | 콘솔 로고·일반 배포 |
| `nexa-clip-64.png` | 64 | 작은 아이콘 |
| `nexa-clip.ico` | 256+64 | Windows 아이콘(멀티 프레임 · PNG 임베드) |

## 색·모티프

- 배경: 청록 그라디언트 `#22C3D6 → #0B7FA6`(앱 `theme.accent` 계열).
- 전경: 흰 클립보드(그라디언트 `#FFFFFF → #E6F8FF`) + accent 집게 + 내용 줄 3개.
- ★ **히스토리 스택**: 보드 뒤로 흰 카드 2장(불투명도 0.68 / 0.38)이 오른쪽으로 계단식.
  **"기록이 쌓인다"** 는 이 제품의 정체성을 아이콘 한 장으로 말한다.
- 모서리 반경 232/1024 · 배경 밖은 투명.

## 재생성 (SVG → PNG/ICO)

`icon.svg`가 원본이다. `rsvg-convert`·ImageMagick이 있으면:

```bash
cd packaging/branding
rsvg-convert -w 1024 -h 1024 icon.svg -o nexa-clip-1024.png
rsvg-convert -w 256  -h 256  icon.svg -o nexa-clip-256.png
rsvg-convert -w 64   -h 64   icon.svg -o nexa-clip-64.png
magick nexa-clip-256.png nexa-clip-64.png nexa-clip.ico
```

⚠️ **현재 PNG/ICO는 도구 없이 생성됐다** — `tools/render-icon.py`(stdlib만 사용하는 최소
래스터라이저: 라운드 사각형 커버리지 + 4×4 박스 다운샘플)로 굽고, ICO는 PNG 임베드 방식
(Vista+)으로 묶었다. 도구가 설치된 환경에서는 위 명령이 정본이며, **결과가 다르면 SVG를 기준**으로 한다.

```bash
python tools/render-icon.py 1024 packaging/branding/nexa-clip-1024.png
```

## 공통 문구

| 항목 | 값 |
| --- | --- |
| App name | `Nexa Clip` |
| Publisher / 개발자 | `SosomLab` (Sangyong Bae · kiros33@gmail.com) |
| Repository | `git@github.com:SosomLab/nexa-clip.git` |
| 한 줄 소개 | 크로스플랫폼 클립보드 매니저 — 복사한 모든 것을 세 OS에서 똑같은 화면으로 꺼내 쓴다 |
