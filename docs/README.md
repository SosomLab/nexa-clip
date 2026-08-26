# docs — 문서 홈

> 4층 문서 체계(진입 / 현황 / 경과 / 지식) — 규약 SSOT는 [16 문서·git 규약](16-doc-git-conventions.md).

## 추천 읽기 순서

1. [../CLAUDE.md](../CLAUDE.md) — 정체성 · 확정 결정 · 규약 · 다음 단계
2. [STATUS.md](STATUS.md) — 지금 무엇이 걸려 있나
   ★ **코드를 쓸 참이면 → [20 구현 설계서](20-implementation-spec.md) 한 장이면 된다**
3. [00 비전](00-vision.md) — **핵심 편의성 8** · 2026 흐름
4. [02 로드맵](02-roadmap.md) — **기능별 목표**
5. [03 경쟁 조사](03-competitive-landscape.md) — **왜 이 제품을 만드는가**(시장 공백 · 차별점)
4. [04 기능 범위 · 화면 구성](04-feature-scope-and-screens.md) — **무엇을 만드는가**
5. [10 결정 기록](10-decision-record.md) — 확정(DR) · **열린 결정(D) 색인**
6. 서식·포맷 → [12](12-clipboard-formats.md)
7. 동기화를 볼 거면 → [05](05-multi-device-sharing.md) → [07](07-device-rendezvous.md) → [08](08-clipboard-propagation.md) → [09](09-identity-and-pairing.md)

## 전 문서

### 현황 · 경과

| 파일 | 성격 | 정렬 |
|---|---|---|
| [STATUS.md](STATUS.md) | 지금 상태 한 장 | 최신 위 |
| [MILESTONES.md](MILESTONES.md) | 기능·목적 관점 현황 | 목표순 |
| [TODO.md](TODO.md) | 순차 백로그 | 목표순 |
| [DEVLOG.md](DEVLOG.md) | 날짜별 **요약** | 시간 역순 |
| [journal/](journal/) | 날짜별 **상세** — 기록의 원본 | 시간 역순 |
| [BRANCHES.md](BRANCHES.md) | 브랜치 이력 | 시간 역순 |

### 지식 (번호 불변 — 재번호 금지, 신규는 뒤에 append)

| # | 문서 | 내용 |
|:--:|---|---|
| 00 | [비전](00-vision.md) | ★ **핵심 편의성 8** · 2026년 최신 흐름과 우리 입장 · 성공 판정 |
| 02 | [로드맵](02-roadmap.md) | ★ **기능별 진행 목표 매트릭스**(M1~M4) · 수용 기준 · 의존 순서 · 리스크 |
| 03 | [경쟁 프로그램 조사](03-competitive-landscape.md) | OS별 클립보드 매니저 전수 · 장단점 · **크로스플랫폼 별도 분석(§5)** · 실사용 기준선 Maccy/CopyQ(§5-4) |
| 04 | [기능 범위 · 화면 구성](04-feature-scope-and-screens.md) | FR 목록(P0~P3) · 화면 S1~S8 + 와이어프레임 · 컨트롤 매핑 · 크레이트 구조 |
| 05 | [다중 기기 공유](05-multi-device-sharing.md) | `nexa-beep` 릴레이 재사용 검토 · 봉투 구조 · **동기화 두 축** |
| 06 | [저장 설계](06-storage-design.md) | **DB vs 파일 직렬화** — 인덱스/blob 분리 · append-only 세그먼트 · 내용 주소 |
| 07 | [기기 랑데부](07-device-rendezvous.md) | **같은 UserId 확인 기법** — 랑데부/인증/소속 3단 · 동시 접속 문제 |
| 08 | [클립보드 자동 전파](08-clipboard-propagation.md) | 타입별 전파 · **함정 3개**(파일 목록·자동 덮어쓰기·에코 루프) · **모바일 수신(§9)** |
| 09 | [신원과 페어링](09-identity-and-pairing.md) | 핸들 vs 키 분리 · **핸들+패스프레이즈 랑데부** · 승인 UX |
| 12 | [클립보드 포맷](12-clipboard-formats.md) | **Rich Text(Word·PPT)** · 원본 형식 보존 · **평문 붙여넣기** · 크로스 OS 정규화 |
| 25 | ★ [디자인 시스템](25-design-system.md) | **Material 골격 + macOS 마감** — 간격·타입·**상태 레이어**·엘리베이션·모션·아이콘 파이프라인 |
| 24 | [참조 설정 화면 연구](24-reference-settings-study.md) | **CopyQ 10장 + Maccy + Paste** 심층 비교 → 우리 설정 화면 방향 |
| 23 | [알파 렌더링](23-alpha-rendering.md) | **반투명** — 앱 안 알파 합성(구현) vs 창 자체 투명(플랫폼 작업·미구현) |
| 22 | 🔴 [beep 전달 원장](22-upstream-beep-liaison.md) | **서버·와이어 변경 연락 창구** — 모든 변경에서 점검하는 체크리스트 + 미전달 항목 |
| 21 | ★ [실기 점검표](21-manual-test.md) | **Windows·macOS·Linux 각각** 무엇이 검증됐는지 · 점검 절차 · 증상 기록 |
| 20 | ★ [구현 설계서](20-implementation-spec.md) | **화면 레이아웃 · 기능 목록 · 각 기능의 구현 방법 · 설정 구성 · 크레이트 배치 · 구현 순서** — 코드 직전에 읽는 한 장 |
| 14 | [설정 레지스트리 명세](14-settings-registry.md) | ★ **Maccy 설정 전수 실측** + 우리 `registry()` 명세(카테고리 11개) |
| 17 | [참조 제품 UI 해부](17-reference-ui-teardown.md) | ★ **Maccy 팝업 · CopyQ 메인창/트레이** 실물 해부 → 우리 설계 정정 |
| 13 | [beep UI 재사용 계획](13-ui-reuse-from-beep.md) | ★ **무엇을 그대로 쓰고 무엇을 버리나** — 재사용 원장(실측 LOC) · 결합 방식 · 착수 순서 |
| 10 | [결정 기록](10-decision-record.md) | **DR 표 + 열린 결정(D) 색인 + 외부 의존 원장 + ADR 색인** |
| 16 | [문서·git 규약](16-doc-git-conventions.md) | 이식용 표준(SSOT) |

### 예약된 번호 (코드 착수 시 생성)

| # | 예정 문서 |
|:--:|---|
| 01 | 아키텍처 (크레이트 구조 SSOT) |
| 11 | 요구사항 (FR/NFR 확정) — ※ 05는 다중 기기가 쓰고 있어 11로 배정 |
| 15 | 개발 방법론 |
| 18 | 빌드 · 테스트 (SSOT) |
| NN | ADR — `NN-adr-000N-*.md` |
