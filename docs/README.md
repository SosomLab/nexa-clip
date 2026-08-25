# docs — 문서 홈

> 4층 문서 체계(진입 / 현황 / 경과 / 지식) — 규약 SSOT는 [16 문서·git 규약](16-doc-git-conventions.md).

## 추천 읽기 순서

1. [../CLAUDE.md](../CLAUDE.md) — 정체성 · 확정 결정 · 규약 · 다음 단계
2. [STATUS.md](STATUS.md) — 지금 무엇이 걸려 있나
3. [03 경쟁 조사](03-competitive-landscape.md) — **왜 이 제품을 만드는가**(시장 공백 · 차별점)
4. [04 기능 범위 · 화면 구성](04-feature-scope-and-screens.md) — **무엇을 만드는가**
5. [10 결정 기록](10-decision-record.md) — 확정(DR) · **열린 결정(D) 색인**
6. 동기화를 볼 거면 → [05](05-multi-device-sharing.md) → [07](07-device-rendezvous.md) → [08](08-clipboard-propagation.md) → [09](09-identity-and-pairing.md)

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
| 03 | [경쟁 프로그램 조사](03-competitive-landscape.md) | OS별 클립보드 매니저 전수 · 장단점 · **크로스플랫폼 별도 분석(§5)** · 실사용 기준선 Maccy/CopyQ(§5-4) |
| 04 | [기능 범위 · 화면 구성](04-feature-scope-and-screens.md) | FR 목록(P0~P3) · 화면 S1~S8 + 와이어프레임 · 컨트롤 매핑 · 크레이트 구조 |
| 05 | [다중 기기 공유](05-multi-device-sharing.md) | `nexa-beep` 릴레이 재사용 검토 · 봉투 구조 · **동기화 두 축** |
| 06 | [저장 설계](06-storage-design.md) | **DB vs 파일 직렬화** — 인덱스/blob 분리 · append-only 세그먼트 · 내용 주소 |
| 07 | [기기 랑데부](07-device-rendezvous.md) | **같은 UserId 확인 기법** — 랑데부/인증/소속 3단 · 동시 접속 문제 |
| 08 | [클립보드 자동 전파](08-clipboard-propagation.md) | 타입별 전파 · **함정 3개**(파일 목록·자동 덮어쓰기·에코 루프) · **모바일 수신(§9)** |
| 09 | [신원과 페어링](09-identity-and-pairing.md) | 핸들 vs 키 분리 · **핸들+패스프레이즈 랑데부** · 승인 UX |
| 10 | [결정 기록](10-decision-record.md) | **DR 표 + 열린 결정(D) 색인 + 외부 의존 원장 + ADR 색인** |
| 16 | [문서·git 규약](16-doc-git-conventions.md) | 이식용 표준(SSOT) |

### 예약된 번호 (코드 착수 시 생성)

| # | 예정 문서 |
|:--:|---|
| 00 | 비전 |
| 01 | 아키텍처 (크레이트 구조 SSOT) |
| 02 | 로드맵 |
| 05x | 요구사항 (FR/NFR 확정) — ※ 05는 사용 중이므로 **11**로 배정 예정 |
| 15 | 개발 방법론 |
| 18 | 빌드 · 테스트 (SSOT) |
| NN | ADR — `NN-adr-000N-*.md` |
