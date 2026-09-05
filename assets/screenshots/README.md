# 화면 캡처 (v0.1.2 · 2026-09-05)

위키·홈페이지용 화면. **macOS · 한국어 UI · Retina 2x**(표시 폭 = 픽셀 폭 ÷ 2) · 타이틀바는 잘라냈다(3-OS 동일 화면이라 OS 창 장식은 뺀다).

- 촬영 방법: 설치본 `--profile docs` 인스턴스(실사용 이력과 분리 · 샘플 8건) → `screencapture -R <창 좌표>` → 타이틀바 56px 제거(`magick -chop`) · 다른 앱이 보이는 영역은 크롭.
- 개인 데이터 없음(샘플 텍스트 · 예시 주소 · 그라디언트 이미지). 트레이 메뉴는 메뉴 바(다른 앱 아이콘)를 잘라냈다.
- 위키 사본: `https://raw.githubusercontent.com/wiki/SosomLab/nexa-clip/images/<이름>.png` (위키 저장소 `images/`).

| 파일 | 내용 |
|---|---|
| `main.png` · `main_detail.png` | 메인창 상세 보기(종류 아이콘 · 출처 앱) + 미리보기 |
| `main_rich.png` · `main_plain.png` | 리치 보기(번호 · 여러 줄 · 섬네일) · 평문 보기 |
| `main_context.png` | 항목 우클릭 메뉴 |
| `main_mode.png` · `main_search.png` | 검색 방식 드롭다운 · 검색 결과 |
| `popup.png` · `popup_search.png` | 퀵 팝업 · 팝업 검색 |
| `tray_menu.png` | 트레이 우클릭 메뉴 |
| `settings_*.png` | 설정 10 카테고리(general · capture · storage · privacy · paste · appearance · shortcuts · search · sync · advanced) |

영문 UI 판이 필요하면(홈페이지) 같은 절차로 `app.lang=en` 프로필에서 다시 찍는다.
