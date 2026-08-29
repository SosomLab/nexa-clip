# BRANCHES — 브랜치 이력

> 시간 역순. 병합·green 확인 후 로컬 브랜치는 삭제하고 이력만 여기 남긴다([16 §4](16-doc-git-conventions.md)).

| 브랜치 | 생성 | 병합 | 커밋 수 | 요약 |
|---|---|---|---:|---|
| `fix/linux-paste-order` | 2026-08-30 | 2026-08-30 | 7 | 실기 라운드 2 — **주입을 팝업 파괴 다음 루프 바퀴로**("첫 번만 붙음" · 사용자 확인 텍스트·이미지 ✅) · ★ **`ui.close_to_tray` 기본 켜짐**(사용자 확정: 닫기=트레이 · Quit만 종료) · **부분 게시 에코 = 원본 승격**(팝업 중복) · ★ **테마 시스템 추종**(beep `theme.rs` 이식 · 포털 실측). 352 테스트 |
| `fix/linux-paste-portal` | 2026-08-30 | 2026-08-30 | 5 | 사용자 실기 셋 — ★ **Wayland 키 주입 = xdg 포털 RemoteDesktop**(ei-portal Xwayland에선 XTest 미도달 · 정정) · ★ **`wl-paste` 폴링 포커스 플랩**(GNOME data-control 부재) → `wayland_probe` + XWayland `xclip` · 타이틀바 두부 → ASCII · Dock 톱니바퀴 → 런처 `.desktop`+아이콘. 349 테스트 |
| `feat/linux-tray-paste` | 2026-08-30 | 2026-08-30 | 5 | ★ **Linux 트레이(SNI/zbus · 최근 항목·알림 · `wlactivate`) + 전역 단축키(xdg 포털 GlobalShortcuts) + K-1 Linux(x11rb XTest · X11 Xvfb 자동 왕복 ✓ · Wayland = 컴포지터 반환 + XWayland) + Linux 클립보드 쓰기 1단(wl-copy/xclip)**. D-Bus 실측 등록·메뉴·재적재·Quit ✓. 사람 실기 ⏳ [21 §2-8](21-manual-test.md) |
| `fix/watch-linux-hardening` | 2026-08-29 | 2026-08-29 | 12 | ★ **Linux 실기 환경 구축 + 결함 여섯** + **4차: X11(XWayland·Xvfb)·Weston 실기 · data-control 실측 · `peek` 거짓 안내 수정** — `watch.rs` git 바이너리(raw NUL) · 빈 클립보드 영구 활동 주기 · 읽기 상한 사후 적용 · **파일 잘라내기 유실** · `MissingTool` 오안내 · ★ **부분 스냅숏**(실기가 잡음). [docs/18 §9](18-build-and-test.md) Linux 환경 문서(배포판별) + [`scripts/linux-watch-e2e.sh`](../scripts/linux-watch-e2e.sh) 자동 실기 |
| *(main 직접)* | 2026-08-27 | — | 2 | ⚠️ **macOS Retina 배율 정정**(`f44cc2b` — `RasterCtx::new`에 배율 필수 인자) · ★ **Windows 클립보드 감시 T-14b**(`08f0a62`). ⚠️ **규약 이탈** — 감시는 큰 단위라 브랜치를 땄어야 했다. 두 세션이 같은 main을 쓰고 있어 브랜치를 나누면 충돌이 잦을 것으로 보고 직접 커밋했으나, **판단 근거를 남기는 것이 규약**이다 |
| *(main 직접)* | 2026-08-26 | — | 2 | 설정 영속 후속 — 점검 화면 저장본 읽기(`152a290`) · ⚠️ **macOS 빌드 복구**(`a823923`, K-1 `appkit` unsafe 블록) + mac 실기 확인. 소규모 fix라 main 직접 |
| *(브랜치 없음 — main 직접)* | 2026-08-26 | — | 3 | ⚠️ **규약 이탈**: `nclip-*` 포크 · **K-1 스파이크** · OS별 실기 점검표를 브랜치 없이 main에 커밋했다. 앞선 병합 직후 main에 남아 있는 것을 못 보고 이어서 작업한 결과다. 다음부터 작업 시작 전 `git branch --show-current` 확인 |
| `feat/scaffold` | 2026-08-26 | 2026-08-26 | 5 | ★ **코드 착수** — 아이콘 · 워크스페이스 8 크레이트 · vendor 무수정 복사 + `VENDOR.lock` · `nclip-core`(i18n·모델·포트) · CI(3-OS + U-1) |
| `docs/design-spec` | 2026-08-26 | 2026-08-26 | 5 | 비전·로드맵 · Rich Text 포맷 · beep UI 재사용 · Maccy/CopyQ 해부 · 설정 레지스트리 · 세로 밀도 레이아웃(DR-14) · **구현 설계서** |
| `docs/initial-research` | 2026-08-25 | 2026-08-25 | 2 | 경쟁 조사 · 기능/화면 · 동기화/저장/랑데부/전파/신원 설계 + 문서 골격 · 라이선스 |
