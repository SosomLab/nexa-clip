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
| ✅ | ★ **K-1 포커스 복원 + 키 주입**(Win 동작 · mac 미검증) · `nclip-plat::{font,paths}` 이식 |
| ✅ | ★ **창 + 렌더 데모** — winit/softbuffer + CPU 래스터라이저 + 보기 3모드 |
| 🚧 | 보관 정책 · 타입 판별 · 중복 승격 파이프라인 |
| 📐 | `nclip-plat` — 클립보드 감시 3-OS(Win 이벤트 · mac 폴링 · X11/Wayland) |
| 📐 | `nclip-plat` — 전역 단축키 · **직전 포커스 창 복원 + 키 주입** · 트레이 · 자동시작 |
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
| 📐 | 페어링(LAN 자동 발견 → 숫자 대조 → 승인) |
| 📐 | **LAN 직결 동기화** ← 사용자 실익이 가장 큰 지점 |
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
