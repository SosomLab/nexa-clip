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

## 4. CopyQ 백엔드 평가 (2026-09-02 소스 정독)

> 구조: `platform/x11/` = x11platform{,clipboard,window}.cpp + `systemclipboard/`(Wayland 폴백).

**의외의 사실 — X11 "직접 구현"의 실체는 Qt 위임이다.** `x11platformclipboard.cpp`는 raw X11
셀렉션 코드가 아니라 **QClipboard 신호 위에 방어층을 쌓은 것**이다: 변화 감지 = Qt `changed()`
신호 + 적응 타이머(50→500ms) 재확인, 3회 재시도 백오프, TIMESTAMP 원자 비교(중복/낡음 판별),
시퀀스 가드(ClipboardDataGuard — 읽는 동안 내용이 바뀌는 경합 탐지), `XQueryPointer`로
"드래그 선택 중이면 대형 읽기 보류" 휴리스틱까지. **X11 셀렉션이 얼마나 경합투성이인지에 대한
20년 치 흉터 목록**으로 읽어야 한다.

**Wayland 폴백 = KDE 코드 사본.** `systemclipboard/waylandclipboard.cpp`는 KGuiAddons
KSystemClipboard의 복제(저작권 헤더가 KDE 개발자)로, **zwlr_data_control만** 구현(ext는 KF6
링크 시에만). 파이프+`poll()` 1초 타임아웃 동기 읽기, SIGPIPE 전용 스레드, **포커스 없을 때
동기 요청 = 교착이라 포커스 감시자를 별도 바인딩**하는 우회가 핵심 복잡도다.

**GNOME은 결국 셸 확장** — D-Bus 클라이언트(`GnomeClipboardExtensionClient`)로 자사 GNOME
Shell 확장에 위임. 공식 알려진 문제: **Flatpak/AppImage에선 확장 등록 불가 = 수집 불가** ·
`QT_QPA_PLATFORM=xcb` 강제 시 "창 닫으면 감시 실패" 부작용.

| 평가 축 | 판정 |
|---|---|
| 견고성 | ★ 상 — 경합·낡음·재시도·보류 처리가 교과서적. 이식 가치 있는 목록 |
| 구조 순수성 | 중 — Qt 의존이 곧 룩 의존(우리 DR-1과 상극) · Wayland는 3중 폴백(KF6 → 자체 zwlr → Qt) 짜깁기 |
| GNOME 답 | 하 — 확장 요구 = 배포 형태 제약 + 사용자 설치 부담. **우리 XWayland 직결이 더 단순** |
| 우리에게의 교훈 | ① 방어층(재시도·TIMESTAMP·읽는-동안-바뀜 가드·드래그 중 보류)은 **P1 설계에 수용** ② Qt층이 없는 우리는 그 버그 우회 3종이 애초에 불필요 ③ 확장 없이 XWayland로 가는 우리 판정이 배포 관점에서 우위 |


## 5. CopyQ vs EcoPaste — 구현·앱 레벨 비교 평가 (2026-09-02)

### 5-1. 구현 — 성능·호환성

| 축 | CopyQ | EcoPaste |
|---|---|---|
| X11 감시 | Qt `changed()` 신호(하부 QXcb=XFIXES) + 방어층(재시도·TIMESTAMP·경합 가드) | `x11-clipboard`(x11rb+XFIXES) — 이벤트 직결. 설계는 더 현대적·깔끔 |
| Wayland | KDE/wlroots data-control + GNOME 셸 확장 — **다 커버**(대가: 3중 폴백 + 확장 배포 제약) | ✗ 없음 |
| ★ Linux 자체 | 3-OS 성숙 · 전 배포판 패키징 | ★ **v1.0 재작성(2026-07)에서 Linux 철회** — v0.x는 X11만 지원했었다. 호환성 평가 자체가 성립 불가 |
| 상주 리소스 | Qt 상주 ~30–50MB(실측 평 기준) — 24h 상주에 정직한 모델 | 설치본 수 MB·"가볍다" 평 — 단 프론트가 OS WebView(Tauri)라 렌더 비용을 WebView에 위임(3-OS 동일 화면도 없음 · [03 차별점 ①③](03-competitive-landscape.md)) |
| **판정** | **종합 우위** — 호환성·견고성·검증 연한 | 백엔드 감시 설계만 보면 우아하나, 그 백엔드조차 이제 Linux에서 안 쓰인다 |

### 5-2. 앱 레벨 — 지명도·다운로드·사용성

| 축 | CopyQ | EcoPaste |
|---|---|---|
| 지명도 | 12.2k★ · 589 fork · 6,901 커밋 · 2013~ 성숙 · 전 배포판 + Flathub ~3.0k/월 | 7.4k★ · 374 fork · **2024-05 창립** 급성장(중국권 기반) |
| 다운로드(GitHub) | v16 55.3k · v15 35.4k · v14 16.3k — 배포판 채널 별도(실제는 훨씬 큼) | v1.1.0 10.9k · v1.0.0 3.7k · v0.6.0-beta.3 89.8k(베타기 인기 정점) |
| 사용성 평판 | 능력 최강이나 **"UI 낡음 · 검색 동선 묻힘 · 학습 곡선"**(리뷰 공통) | **"현대적 · 직관 · 즉시 사용"** 호평 · OCR 등 |
| 활동성 | 현역(v16 2026-05) | 현역(nightly 활발) — 단 이슈 100+ · 신생 리스크(03 §5-1 판정 유지) |

### 5-3. 우리에게의 시사점

1. 시장은 **"능력(CopyQ) vs 현대성(EcoPaste)"로 분단** — 둘 다 가진 제품이 없다. 우리 한 줄(*"Ditto의 능력 · Maccy의 가벼움 · CopyQ의 이식성"*)의 자리가 그대로 비어 있다.
2. ★ **EcoPaste의 Linux 철회는 WebView 진영의 Linux 감당 실패 방증** — Tauri(WebKitGTK)+클립보드 파편화를 신생 팀이 유지 못 했다. 자체 래스터라이저 + 직접 구현(DR-1) 노선의 반사이익.
3. CopyQ의 진짜 교훈은 §4의 방어층 목록이고, EcoPaste의 진짜 교훈은 하부 크레이트(x11rb+XFIXES) 선택이다 — **우리 P1은 이 둘의 합집합**을 새 crate 0으로 갖는다.


## 6. 구현 설계 권장안 (T-14 본편 · P1/P2)

> §1~§5의 수렴 — **x11rb 직접 구현**(EcoPaste의 크레이트 선택) + **CopyQ의 방어층**을
> 새 crate 0으로. 참조 선례 = `x11-clipboard`(MIT · x11rb 0.13 + xfixes — 우리 lock과 같은 판).

### 6-1. 모듈·스레드 구조

- 신설 `nclip-plat/src/selection_x11.rs` — **연결 2개**(x11-clipboard 선례): ① 감시·읽기(요청자) ② 쓰기 서빙(소유자). 하나로 합치면 자기 셀렉션을 자기가 읽을 때 교착한다.
- 각 연결 = 전용 스레드 + 숨은 창. 서빙 스레드는 이벤트 fd + 명령 채널을 `poll()`로 겸청.
- 사다리 개정: ① Wayland+data-control → `wl-paste` 유지(P3까지) ② `DISPLAY` → **x11rb 직접** ③ x11rb 연결 실패 → 기존 도구 파이프 폴백(정직 강등 사유 보고) ④ 전무 → 사유.

### 6-2. 감시·읽기 (P1)

- `XFixesSelectSelectionInput(CLIPBOARD, owner-change)` → 이벤트마다: SETTLE 재확인(기존 규칙 유지) → `TARGETS` 변환 → 표현별 `ConvertSelection`.
- **INCR 수신**: 프로퍼티 type=INCR → 삭제로 다음 청크 요청 → 조립. ★ `MAX_REP_BYTES`를 **청크 도중** 집행(초과 = 중단 + 이름만 — 기존 규칙 동일).
- **CopyQ 방어층 수용**: 변환 실패(property=None) 재시도 ≤3 · TIMESTAMP 비교로 중복 스킵 · 스냅숏 확정 전 소유자/타임스탬프 재확인(읽는-동안-바뀜 가드).
- ★ **에코 판정 승격**: 소유자 창 == 우리 서빙 창이면 자기 게시 — 지문 비교보다 정확(08-30 에코 승격 규칙의 X11판).
- 민감 표식: TARGETS에 `x-kde-passwordManagerHint` 있으면 값 확인 · 못 읽으면 금지(fail-closed 유지).
- 기존 `normalize_target`·`is_meta_target`·타깃 정리 로직 그대로 재사용(순수 함수 — 테스트 승계).

### 6-3. 쓰기 서빙 (P2)

- `SetSelectionOwner` 전 **TIMESTAMP 취득**(zero-append PropertyNotify 트릭 — CurrentTime 금지·ICCCM).
- `SelectionRequest` 응대: TARGETS(★ **전 표현 광고 = 다중 표현 게시** — 1단의 "1개" 제약 해소) · TIMESTAMP · MULTIPLE · 표현별 데이터. text/plain ↔ UTF8_STRING/STRING/TEXT 매핑.
- **INCR 송신**: `maximum_request_bytes()` 초과분은 요청자별 전송 상태 + PropertyNotify(delete) 구동 청크. 정체 전송은 타임아웃 GC.
- `SelectionClear` = 소유권 상실 = 남이 복사 — 감시 신호와 합류.

### 6-4. 검증·게이트

- Xvfb 왕복(K-1 선례): 소형 왕복 · **INCR 왕복(≥1MB)** · TARGETS 광고 · UTF8 매핑 · 소유권 상실 이벤트 · 상한 중단. `clip_roundtrip` 확장.
- 의존 원장: x11rb `xfixes` **feature 추가**(새 crate 0) — [10 §3](10-decision-record.md) 기록.
- x11-clipboard 차용 시 MIT 고지. 통째 vendoring보다 **우리 규약(cap·정직 강등·이벤트 브리지)에 맞춘 재작성** 권장(~600~900줄 추정).

### 6-5. 사용자 확정 대기(일괄)

| # | 질문 | 권장 |
|---|---|---|
| ① | 도구 파이프 폴백 유지? | 유지 — x11rb 연결 실패·특수 환경의 정직 강등 계단 |
| ② | PRIMARY(마우스 선택) 수집? | 범위 밖(CLIPBOARD만) — Klipper·CopyQ는 옵션 제공. 열린 결정으로 등재만 |
| ③ | 종료 시 클립보드 내용? | 우리가 소유 중 종료하면 X11 특성상 소멸 — 종료 직전 재게시 없이 수용(우리가 곧 매니저) · 문구로 명시 |


## 7. 출처

- CopyQ: [x11platformclipboard.cpp](https://github.com/hluk/CopyQ/blob/master/src/platform/x11/x11platformclipboard.cpp) · [Known Issues(GNOME 확장·Wayland)](https://copyq.readthedocs.io/en/latest/known-issues.html)
- Klipper/KSystemClipboard: [KDE MR !1(Wayland 포팅)](https://invent.kde.org/plasma/plasma-workspace/-/merge_requests/1) · [wlr-data-control](https://wayland.app/protocols/wlr-data-control-unstable-v1)
- GPaste: [Mutter MR !320(클립보드 매니저 논의)](https://gitlab.gnome.org/GNOME/mutter/-/merge_requests/320)
- EcoPaste: [tauri-plugin-clipboard-x](https://github.com/ayangweb/tauri-plugin-clipboard-x) · [clipboard-rs](https://github.com/ChurchTao/clipboard-rs) · [x11-clipboard(x11rb+xfixes 실증)](https://github.com/quininer/x11-clipboard)
- CopyQ Wayland 폴백: [waylandclipboard.cpp(KDE 사본·zwlr)](https://github.com/hluk/CopyQ/blob/master/src/platform/x11/systemclipboard/waylandclipboard.cpp)
- 앱 지표: [CopyQ repo(12.2k★)](https://github.com/hluk/CopyQ) · [EcoPaste repo(7.4k★ · README에서 Linux 소멸)](https://github.com/EcoPasteHub/EcoPaste) · [EcoPaste Linux 이슈 #75](https://github.com/ayangweb/EcoPaste/issues/75) · GitHub API 릴리스 다운로드 실측(09-02) · [Flathub CopyQ](https://flathub.org/apps/com.github.hluk.copyq)
- 사용성 평판: [DEV 2026 비교 리뷰](https://dev.to/abhijith_p_subash/best-clipboard-managers-in-2026-mac-windows-linux-compared-i-tested-them-all-8a3)
- 진영 지형: [ArchWiki Clipboard](https://wiki.archlinux.org/title/Clipboard) · [Hyprland Wiki(cliphist)](https://wiki.hypr.land/Useful-Utilities/Clipboard-Managers/)
