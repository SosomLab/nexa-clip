# STATUS — 지금 상태 한 장

> 시간 역순. 상세는 [journal](journal/), 여기는 요약.

---

## 2026-08-25 (1차) — 설계·조사 완료 · **코드 미착수**

**한 줄**: 경쟁 조사부터 신원·페어링 설계까지 **지식 문서 7건**을 세웠고, 이제 **병목은 코드가 아니라 결정**이다.

### 지금 걸려 있는 것

★ **열린 결정 29건 중 11건이 "권장안 제출 · 사용자 확정만 남음"** 상태다([10 §2](10-decision-record.md#2-열린-결정-d--색인)).
그중 **넷이 나머지를 좌우한다**:

| # | 결정 | 권장 | 왜 먼저인가 |
|:--:|---|---|---|
| **D-20** | 자동 덮어쓰기 기본값 | ①기록만 기본 + ②·③ 설정 | 사용자 요구는 ②에 가까운데 **상대 PC 작업 파괴 위험**이 실재. W-1(원본 보존)+W-3(되돌리기)를 붙이면 ②도 안전 |
| **D-9** | 디스크 at-rest 암호화 | **기본 켜짐** | 평문이면 **현재 쓰는 CopyQ보다 후퇴**하고 차별점 ②가 사라진다 |
| **D-1** | 저장 구조 | **자체 파일 직렬화** | 크레이트 구조·검색 방식이 여기서 갈린다 |
| **D-24** | 랑데부 파생 재료 | **핸들 + 패스프레이즈** | 신원·페어링 UX 전체가 여기에 달림 |

### 완료 (08-25)

- ✅ 라이선스 구성 — `nexa-beep`와 동일(PolyForm NC 1.0.0 + 한글본 + `.gitattributes`). 프로젝트명·URL 4곳만 치환
- ✅ [03 경쟁 조사](03-competitive-landscape.md) — OS별 24종 + 크로스플랫폼 6종. **사실 2건 정정**(CopyQ 암호화 존재 · Maccy 2.x 이미지 지원)
- ✅ [04 기능·화면](04-feature-scope-and-screens.md) — FR 8분류 · 화면 S1~S8 · 컨트롤 매핑 · 크레이트 초안
- ✅ [05 다중 기기](05-multi-device-sharing.md) · [07 랑데부](07-device-rendezvous.md) · [08 자동 전파](08-clipboard-propagation.md) · [09 신원·페어링](09-identity-and-pairing.md)
- ✅ [06 저장 설계](06-storage-design.md) — D-1 답(파일 직렬화 + 3규칙)
- ✅ 문서 골격 — CLAUDE.md · README · STATUS/DEVLOG/journal/MILESTONES/TODO/BRANCHES · [10 DR](10-decision-record.md) · [16 규약](16-doc-git-conventions.md)

### 이번에 확인된 사실 (재조사 금지 — 실코드 근거)

| 사실 | 근거 |
|---|---|
| ★ `nexa-beepd`의 RID 맵이 **`HashMap<Rid, ConnId>` = 1 RID 1 연결** → **공유 URID 안이 깨진다** | `crates/nexa-beepd/src/lib.rs:127,459` |
| ★ 릴레이는 **"버퍼가 아니라 파이프"** — 양쪽 동시 접속일 때만 흐른다 → **오프라인 큐 없음** | `crates/nexa-beepd/src/lib.rs:12` |
| `nbeep-relay` 1,769 LOC · `nexa-beepd` 951 LOC — **릴레이 MVP 구현 완료** | 실측 `wc -l` |
| beep roster(`Announce`/`PeerUp`)는 **PeerId 공개키 원본을 노출** → 다중 기기용으로 쓰면 안 됨 | `crates/nbeep-relay/src/lib.rs` |
| beep에서 **컨텐츠 서버(모드 ②)·다중 기기(ADR-0007) 모두 미구현** | `docs/32`(Proposed) · `docs/20`(v2) |

### 다음

1. 위 4건 결정 확정 → [10 §1](10-decision-record.md#1-확정-결정-dr) DR 표로 이동
2. ADR 승격(D-1 · D-24 · D-20)
3. 요구사항 문서 + 스택 ADR(예산 수치)
4. 코드 착수 — 워크스페이스 + `nclip-core` + `nclip-plat` 수직 슬라이스
