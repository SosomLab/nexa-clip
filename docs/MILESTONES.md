# MILESTONES — 기능·목적 관점 현황

> ✅ 완료 / 🚧 진행 / 📐 설계만(코드 없음) / ☐ 미착수. 목표순.

## M0 — 설계·조사

| 상태 | 항목 | 근거 |
|:--:|---|---|
| ✅ | 경쟁 조사(OS별 + 크로스플랫폼) · 차별점 확정 | [03](03-competitive-landscape.md) |
| ✅ | 기능 목록(FR) · 화면 구성 · 컨트롤 매핑 | [04](04-feature-scope-and-screens.md) |
| ✅ | 저장 설계(D-1 답) | [06](06-storage-design.md) |
| ✅ | 다중 기기 · 랑데부 · 전파 · 신원 설계 | [05](05-multi-device-sharing.md) [07](07-device-rendezvous.md) [08](08-clipboard-propagation.md) [09](09-identity-and-pairing.md) |
| ✅ | 문서 골격 · 라이선스 | [10](10-decision-record.md) [16](16-doc-git-conventions.md) |
| ✅ | **비전(핵심 편의성 8) · 로드맵(기능별 목표)** | [00](00-vision.md) [02](02-roadmap.md) |
| ✅ | **Rich Text·포맷 설계** · 화면 구성 확정 | [12](12-clipboard-formats.md) [04 §2](04-feature-scope-and-screens.md) |
| ✅ | ★ **beep UI 재사용 계획**(실측 ≈21,600 LOC) | [13](13-ui-reuse-from-beep.md) |
| ✅ | **참조 제품 UI 해부**(Maccy·CopyQ) · **설정 레지스트리 명세** | [17](17-reference-ui-teardown.md) [14](14-settings-registry.md) |
| ✅ | ★ **구현 설계서** — 기능별 구현 방법 · 크레이트 배치 · 수직 슬라이스 11단계 | [20](20-implementation-spec.md) |
| ✅ | **레이아웃 재확정**(세로 밀도 우선 · 좌측 세로 툴바) | [04 §2-2](04-feature-scope-and-screens.md) · DR-14 |
| 🚧 | **핵심 결정 확정**(D-20·D-9·D-1·D-24) | [10 §2](10-decision-record.md#2-열린-결정-d--색인) |
| ☐ | 요구사항 문서(FR/NFR) · 스택 ADR(예산 수치) | — |

## M1 — 로컬 완결 제품 (v1 · 동기화 없음)

| 상태 | 항목 |
|:--:|---|
| ✅ | **워크스페이스 8 크레이트**(`nclip-*` 통일 · DR-17 포크) + CI(3-OS) |
| ✅ | `nclip-core` — i18n · **항목/다중 표현 모델** · 포트(`ClipboardWatch`·`WatchCapability`) |
| ✅ | `nclip-plat::watch` 게이트(민감 표식 > 일시정지 > 다음 1건) · `ViewMode` |
| ✅ | ★ **K-1 키 주입**(Win 실사용 통과 · **복원 경로는 미검증** · mac **빌드 복구됨** · 실기 미검증) · `nclip-plat::{font,paths}` 이식 · ★ **Linux/Wayland 안정화(09-05)** — 포털 RemoteDesktop **세션 자가 복구**(셸이 세션을 닫아도 저장 토큰으로 재개설 · 재현 테스트) + **승인 토큰을 빌드 무관 고정 자리로**(개발↔설치본을 오가도 재승인 없음 · 실측 0.55~0.57s 무대화창) |
| ✅ | ★ **창 + 렌더 데모** — winit/softbuffer + CPU 래스터라이저 + 보기 3모드 + 알파 합성 |
| ✅ | ★ **설정 화면(S3)** — 이식 프레임워크 2,000줄 + 레지스트리 21항목 + **검색·즉시 적용·스플리터** |
| ✅ | 진단 로그(`diag`) — 원인+조치 · 링 버퍼 |
| ✅ | ★ **디자인 토큰** — 간격·타입·상태 레이어·엘리베이션·모션([25](25-design-system.md) 코드화) |
| ✅ | ★ **hover 페이드**(트리 행·버튼·콤보 · 1000ms) — [DR-34](10-decision-record.md) |
| ✅ | ★ **설정 영속**(`nexa-conf` · 지연 저장 · 미지 키 보존) |
| ✅ | ★ **붙여넣기 모드 4개**(원본/평문/객체로/경로만 · `PasteAs::applicable`) — [DR-35](10-decision-record.md) |
| 🚧 | 남은 컨트롤에 hover 적용(스위치·색상·툴바·풀다운) |
| 🚧 | 보관 정책 · 타입 판별 · 중복 승격 파이프라인 |
| ✅ | `nclip-plat` — 클립보드 감시: **Windows ✅**(이벤트) · **mac ✅**(08-29 changeCount 폴링) · **Linux ✅**(09-03 · x11rb+XFIXES 내재화 · Wayland = XWayland 브리지 · 텍스트 타깃 UTF-8 순위) — 잔여: KWin/Sway data-control 내재화(P3) |
| ✅ | ★ **`watch`·`peek` 진단 명령** — 복사한 것이 무엇으로 잡히는지 실기로 본다 |
| ✅ | ★ **실기 18건 회귀 박제**(PPT·Excel·Edge·VS Code·Greenshot·CopyQ) |
| ✅ | ★ **캡처 파이프라인** — 종류 판정 · `Preview` · 용량 규칙([27](27-capture-cases.md) → `nclip-core::capture`) |
| 🚧 | `nclip-plat` — 전역 단축키 · **직전 포커스 창 복원 + 키 주입** · 트레이 · 자동시작 — **Win ✅ · Linux ✅(08-30 · SNI/포털/XTest · ★ 단축키 사용자 실기 ✓ 09-05 · Wayland 네이티브 앱 실기 ⏳) · ★ mac 트레이 ✅(09-04 · NSStatusItem + Dock 정책) · mac 단축키 ☐** |
| ✅ | ★ **창을 앞으로(Linux/X11 · 09-05)** — winit `focus_window()`가 `_NET_ACTIVE_WINDOW`를 소스=1(앱)로 보내 Mutter 포커스 탈취 방지에 막히던 것("준비됨" 알림 + 창이 뒤에) → **페이저 소스(=2)로 직접 올림**. 메인창·설정 창 · 신규/재표시 **공통 길목**(`bring_to_front`) · Mutter 존중 실증 테스트 |
| ✅ | ★ **설치본 자리에서 실기(3-OS 갖춤 · 09-05)** — Linux `scripts/dev-install-linux.sh`(win `dev-install-win.ps1` · mac `dev-install-mac.sh`의 Linux 판): 배포 `.deb` 설치본(`/usr/bin`)에 릴리스 빌드를 원자 교체 → **설치 `.desktop` 경로 지정 실행**(직접 실행·systemd-run은 포털이 앱을 식별 못 해 전역 단축키 등록 실패) · 데이터 `~/.config/nexa-clip` |
| ✅ | ★ **배포 3회 — v0.1.0·v0.1.1(09-04) · v0.1.2(09-05)**: 태그 push → 5타깃 패키지(linux deb/tar · mac x64/arm64 dmg+portable · win x64/arm64 setup+portable) + SHA256SUMS → GitHub Release 공개 → brew 탭 자동 갱신. **winget/choco는 변수(`WINGET_PUBLISH`·`CHOCO_PUSH`=false)로 제출 제외** — 매니페스트만 자산에 동봉 · 검수 대기 자동 판정(guard) 갖춤 |
| ✅ | ★ **Linux 전역 단축키 실사용 성립(09-05 · 사용자 실기 ✓)** — 근인 둘을 걷어냈다: ① 포털은 **cgroup 유닛에서 앱 이름을 읽는다** → 에디터 터미널에서 띄우면 `code`로 등록돼 정상 실행분이 충돌 실패(스코프 이름의 하이픈은 `\x2d` 이스케이프 필수) ② 앱이 시작마다 사용자 런처를 **자기 경로로 덮어써** 개발 빌드가 아이콘·자동 실행·단축키를 가로챘다 → **패키지가 설치돼 있으면 사용자 런처를 설치본을 가리키게 유지**(지우면 gnome-shell이 삭제된 경로를 캐시해 실패). ⚠️ 런처 경로 변경은 **셸 재시작까지가 절차**(인메모리 캐시). 리부팅 후 실측 = 아이콘·자동 실행·단축키 모두 설치본 |
| 📐 | `nclip-store` — 암호화 세그먼트 + 내용 주소 blob |
| 📐 | ★ **beep UI 이식** — `gfx`·`ctl` 무수정 복사 · **설정 화면 프레임워크**(registry만 교체) · 트레이 · 중립 모듈 |
| 📐 | **S1 퀵 팝업**(타입어헤드 · 한글 조합 검색 · 자동 붙여넣기) |
| 📐 | ★ **S2 메인창** — 메뉴+검색 1줄 통합 · **좌측 세로 툴바(40px)** · **보기 3모드**(일반/간략/한 줄) |
| 📐 | ★ **S6 트레이 우클릭 — 최근 5~10개 직접 표시** |
| 📐 | ★ **Rich Text 원본 포맷 보존**(Office/GVML) + **평문 붙여넣기** |
| 📐 | **제한 리치텍스트 렌더러**(굵기·색·표·목록 — 스크립트·네트워크 0) |
| 📐 | S3 설정 · S7 온보딩 |
| 📐 | 다크·고DPI·다국어(ko/en/ja) |

## M2 — 기기 간 동기화 (2차)

| 상태 | 항목 |
|:--:|---|
| 📐 | `UserId` 키 + `DeviceList`(ADR-0007 **첫 실구현**) |
| 🚧 | 페어링(LAN 자동 발견 → 숫자 대조 → 승인) — **UDP 비콘 발견 + 기기별 승인 행·6자리 대조 ✅(09-04)** · 페어링 창·DeviceList 서명 ☐ |
| 🚧 | **LAN 직결 동기화** ← 사용자 실익이 가장 큰 지점 — **TCP 직결 1단 ✅(09-04)** · ★ **두 PC 세션 성립 ✅(09-05 · mac 호스트 ↔ Linux VM)** · 릴레이 None 자동 재기동 ✅ · ⚠️ 인터페이스 열거(가상 어댑터 · 비콘 한 방향 실증) ☐ · 승인→전파 실기 ⏳ · ★ **설치본 기준 실기 환경 확립(09-05)** — 배포 `.deb`로 설치해 `/usr/bin` + `~/.config/nexa-clip`에서 mac과 LAN 세션 성립(이력 38 · 단축키 · 키 주입 ok) |
| 📐 | 릴레이 경유(원격) — `nexa-beepd` 재사용 |
| 📐 | 자동 전파(에코 차단 · 전순서 · 덮어쓰기 정책) |
| ☐ | 신뢰 기기와 **공유**(수동·보드 단위) |

## M3 — 확장 (후순위)

| 상태 | 항목 |
|:--:|---|
| ☐ | 변환 규칙(정규식 치환 등) · 스니펫 심화 |
| ☐ | 오프라인 큐(컨텐츠 서버) |
| ☐ | CLI 제어 |
| ☐ | **Android·iOS 수신**(P3 · 최하순위) |
