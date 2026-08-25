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
| 🚧 | **핵심 결정 확정**(D-20·D-9·D-1·D-24) | [10 §2](10-decision-record.md#2-열린-결정-d--색인) |
| ☐ | 요구사항 문서(FR/NFR) · 스택 ADR(예산 수치) | — |

## M1 — 로컬 완결 제품 (v1 · 동기화 없음)

| 상태 | 항목 |
|:--:|---|
| 📐 | 워크스페이스 + `nclip-core` 항목 모델 |
| 📐 | `nclip-plat` — 클립보드 감시 3-OS(Win 이벤트 · mac 폴링 · X11/Wayland) |
| 📐 | `nclip-plat` — 전역 단축키 · **직전 포커스 창 복원 + 키 주입** · 트레이 · 자동시작 |
| 📐 | `nclip-store` — 암호화 세그먼트 + 내용 주소 blob |
| 📐 | `nclip-gfx`/`nclip-ctl` 이식(beep) + `grid`/`columns` 이식(dir2) |
| 📐 | **S1 퀵 팝업**(타입어헤드 · 한글 조합 검색 · 자동 붙여넣기) |
| 📐 | S2 라이브러리 · S3 설정 · S6 트레이 · S7 온보딩 |
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
