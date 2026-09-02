# 29 · Linux 클립보드 접근 실태 — 경쟁 조사 · 직접 감시 vs 도구 파이프

> 2026-09-02 사용자 질문 셋("xclip 없이 가능한가" · "경쟁이 xclip을 쓰는가" · "직접 감시와 xclip의 차이")에
> 대한 조사·분석. **T-14 본편(Linux 감시 내재화)의 입력 문서**다. 결론 먼저 —
> **GUI 경쟁 제품 중 xclip/wl-paste 파이프를 쓰는 곳은 0/9다.** 도구 파이프는
> 미니멀 스크립트 진영(cliphist 등)의 관례이고, 성숙 제품은 전부 프로토콜 직접 구현이다.

## 1. 경쟁 제품별 접근 방식 (2026-09-02 실증)

| 제품 | 계층 | X11 접근 | Wayland 접근 | xclip류 사용 |
|---|---|---|---|---|
| **CopyQ** | C1 크로스 | ★ **자체 X11 백엔드**(`src/platform/x11/x11platformclipboard.cpp`) | KF6 `KSystemClipboard`(ext→wlr data-control) · 자체 폴백은 wlr만 · ★ **GNOME Wayland = XWayland 강등 + 전용 GNOME Shell 확장**(감시용) | ✗ |
| **EcoPaste** | C2 크로스 | `tauri-plugin-clipboard-x` → **`clipboard-rs`** → `x11-clipboard`(**x11rb 0.13 + xfixes** — 우리 lock과 같은 판) | ✗ 미지원(문서상 Linux(X11)만) | ✗ |
| **PasteBar** | C3 크로스 | Linux 지원 자체 미확인(03 §5-1) | — | — |
| **Klipper** | KDE 기본 | Qt(QClipboard) | ★ `KSystemClipboard` — `ext_data_control_v1` 우선 → `zwlr_data_control_v1` → Qt 폴백 | ✗ |
| **GPaste** | GNOME | GTK/GDK 직접(소유자 변경마다 재검사) | ★ **GNOME Shell 확장 = 컴포지터 내부 특권**으로 해결(프로토콜 부재를 우회) | ✗ |
| **Clipman** (xfce4) | XFCE | GTK(GtkClipboard) | X11 중심 | ✗ |
| **Diodon·Parcellite·ClipIt** | GTK 구세대 | GTK(GtkClipboard — libX11 경유) 직접 | 취약(사실상 X11 전용) | ✗ |
| **greenclip** | 미니멀 | Haskell X11 바인딩 직접 | — | ✗ |
| **cliphist · clipse · clipvault** | Wayland 미니멀 | — | ★ **`wl-paste --watch` 파이프**(wl-clipboard 위임) — **우리 1단과 같은 방식** | ✅ (wl-paste) |

**관찰 셋**:
1. **성숙 GUI 제품은 전부 직접 구현** — 도구 파이프는 "자체 UI 없는 스크립트 진영"의 관례다. 우리 1단(도구 파이프)은 검증용 계단으로는 옳았지만 제품 체급에선 소수파.
2. **GNOME Wayland는 만인의 벽** — CopyQ는 XWayland 강등+Shell 확장, GPaste는 아예 Shell 확장(컴포지터 내부)이 됨. 우리의 XWayland 경로 판정(08-30)은 CopyQ와 동일 결론.
3. ★ **EcoPaste의 하부(`x11-clipboard`)가 정확히 우리 P1 설계다** — x11rb 0.13 + `xfixes` feature, XFIXES `SelectionNotify` 감시(+50ms 폴링 폴백), INCR 양방향, 소유권 상주 스레드로 쓰기 서빙. 같은 x11rb 판이라 **차용/이식 검토 가치**(MIT — 라이선스 확인 후).

## 2. 직접 감시 vs xclip 파이프 — 차이 분석

| 축 | 도구 파이프 (현 1단) | 직접 구현 (본편 · x11rb+XFIXES) |
|---|---|---|
| **변화 감지** | 신호 없음 → **폴링**(500ms~2s) + FNV 지문 비교. 틱마다 프로세스 spawn + 전량 읽기 | ★ **이벤트 푸시**(`XFixesSelectSelectionInput` → `SelectionNotify`). 복사 순간 정확히 1회 반응 |
| **지연** | 최대 폴링 주기만큼(유휴 2s) — 빠른 연속 복사를 놓칠 수 있다(중간 상태 미관측) | 소유자 변경마다 통지 — 연속 복사도 전부 관측 |
| **상주 비용(DR-9)** | 틱당 fork/exec ×(타깃 수+1) + 파이프 왕복 — 유휴에도 CPU 소모 | 연결 1개 유지 · 유휴 비용 ≈ 0. **예산 게이트 정합** |
| **표현(쓰기)** | `-t` 하나 → **대표 표현 1개만 게시**(현 제약) | ★ 소유권 직접 서빙 → **다중 표현 게시**(TARGETS에 전부 광고) — T-14 잔여 해소 |
| **INCR(대용량)** | 도구가 대행(우리는 파이프 상한만) | 양방향 직접 처리 필요 — **최대 복병**(수신 조립·송신 분할). 선례: x11-clipboard |
| **수명/원자성** | 쓰기마다 xclip이 fork 상주 — 프로세스 잔존·경합 관리 밖 | 우리 스레드가 소유 — 24시간 상주 앱과 자연 정합. 종료 시 재게시 정책도 우리 손에 |
| **실패 양상** | 도구 부재 = 기능 전체 불가(지금 이 PC) · 도구 버전/동작 차 관리 밖 | 배포 의존 0(순수 Rust · libxcb 불요) — **설치 안내 자체가 소멸** |
| **민감 표식** | 타깃 값 읽기 왕복 추가 | TARGETS 응답에서 즉시 판정 |
| **난이도** | ✅ 낮음(이미 있음) | 중~대 — ICCCM 셀렉션 규약(TARGETS·TIMESTAMP·MULTIPLE·INCR) 직접 이행 |

**한 줄** — 파이프는 "남의 상주 프로세스를 빌려 폴링"하는 것이고, 직접 구현은 "우리가 그 상주 프로세스가 되는" 것이다. 클립보드 **매니저**는 어차피 후자가 본업이다.

## 3. 시사점 → 단계 (검토 09-02 · [TODO T-14](TODO.md))

1. **P1 — 감시·읽기 내재화**: x11rb `xfixes` feature 추가(의존 원장 기록) · XFIXES 이벤트 감시 · `XConvertSelection`+INCR 수신. 폴링·spawn 소멸.
2. **P2 — 쓰기 서빙**: 소유권 상주 스레드 + TARGETS/MULTIPLE/INCR 송신 → xclip 완전 제거 + 다중 표현.
3. **P3(선택) — Wayland 내재화**: `wayland-protocols 0.32(staging)`의 `ext-data-control-v1`로 KWin/Sway용 wl-clipboard 제거. **GNOME 부재만 불가항력** → XWayland 경로가 메운다(경쟁도 동일).

## 4. 출처

- CopyQ: [x11platformclipboard.cpp](https://github.com/hluk/CopyQ/blob/master/src/platform/x11/x11platformclipboard.cpp) · [Known Issues(GNOME 확장·Wayland)](https://copyq.readthedocs.io/en/latest/known-issues.html)
- Klipper/KSystemClipboard: [KDE MR !1(Wayland 포팅)](https://invent.kde.org/plasma/plasma-workspace/-/merge_requests/1) · [wlr-data-control](https://wayland.app/protocols/wlr-data-control-unstable-v1)
- GPaste: [Mutter MR !320(클립보드 매니저 논의)](https://gitlab.gnome.org/GNOME/mutter/-/merge_requests/320)
- EcoPaste: [tauri-plugin-clipboard-x](https://github.com/ayangweb/tauri-plugin-clipboard-x) · [clipboard-rs](https://github.com/ChurchTao/clipboard-rs) · [x11-clipboard(x11rb+xfixes 실증)](https://github.com/quininer/x11-clipboard)
- 진영 지형: [ArchWiki Clipboard](https://wiki.archlinux.org/title/Clipboard) · [Hyprland Wiki(cliphist)](https://wiki.hypr.land/Useful-Utilities/Clipboard-Managers/)
