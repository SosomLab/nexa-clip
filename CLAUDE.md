# CLAUDE.md — Nexa Clip 프로젝트 컨텍스트 (이식용 메모리)

> 이 파일은 **다른 PC에서 clone 시 즉시 컨텍스트를 공유**하기 위한 휴대용 프로젝트 메모리다.
> **먼저 읽기:** [docs/STATUS.md](docs/STATUS.md)(현황) → [docs/10-decision-record.md](docs/10-decision-record.md)(결정).

## 1. 이 프로젝트는

**Nexa Clip** = **크로스플랫폼 클립보드 매니저**(Windows · macOS · Linux).
**올 러스트 · 단일 바이너리 · 자체 CPU 래스터라이저**로 **3-OS 완전 동일 화면**을 그린다 — Qt·WebView·Electron을 쓰지 않는다.

- 조직: **SosomLab** · 개발자: Sangyong Bae · kiros33@gmail.com
- 저장소: <https://github.com/SosomLab/nexa-clip> · 라이선스: **PolyForm Noncommercial 1.0.0**
- 현 단계: ★ **M2 진행 중 · v0.1.1 배포됨**(09-04) — M1(감시·캡처·암호화 영속·팝업·메인창·설정·트레이·주입 3-OS) 완료 · 동기화(릴레이+LAN 직결·기기 승인·전파) · 리치 렌더 2단 · 검색(색인·정규식) · 메모리 상주 계층(DR-42) · 배포 파이프라인(brew · winget/choco는 변수로 제외). 핵심 결정은 DR-42까지 확정.

### 참조 원천 (재발명 금지 — 설계 전 반드시 확인)

| 원천 | 로컬 경로 | 무엇을 가져오나 |
|---|---|---|
| **`nexa-beep`** | `../nexa-beep` | ★ **기본 틀** — 크레이트 경계 · `plat` 포트 · CPU 래스터라이저(`nbeep-gfx`) · 컨트롤(`nbeep-ctl`) · 클립보드 어댑터(`nbeep-plat/clipboard.rs`) · 트레이 · **릴레이(`nbeep-relay`·`nexa-beepd`)** · 암호화 키 계층(ADR-0005) · 다중 기기 신원(ADR-0007) |
| **`nexa-dir2`** | `../nexa-dir2` | ★ **컨트롤** — `ctl` 17종 · `nexa-gui`(`draw`·`event`·`geom`·`theme`·`edit`·`typeahead`) · **`grid`/`columns`**(정렬·리사이즈) · 가상화 목록 |

## 2. 확정 결정 (요약 — 전문은 [docs/10](docs/10-decision-record.md))

| # | 결정 |
| --- | --- |
| DR-1 | **자체 CPU 래스터라이저로 직접 그린다** — 3-OS 동일 화면. 프레임워크 룩 금지 |
| DR-2 | 컨트롤 계약 = `nexa-dir2` `ctl` + `nexa-beep` `nbeep-ctl` 계승. **시각은 한 벌, 동작 관례는 각 OS 네이티브** |
| DR-3 | 라이선스 = PolyForm NC 1.0.0 (beep와 동일 구성) |
| DR-4 | **전송은 봉인**(Noise E2E · 서버는 봉투만), **로컬은 해제해 관리** |
| DR-5 | 동기화 두 축 — ① 같은 `UserId` 기기 **자동 연동** · ② 신뢰해 등록한 기기와 **공유**(수동) |
| DR-6 | 복사 즉시 **자동 전파**(LAN + 원격). **파일은 경로 목록만** |
| DR-7 | **Android·iOS는 수신만** · **최하순위(P3)**. 모바일 자동 캡처는 범위 밖(OS 제약) |
| DR-8 | 외부 crate 기본 0 지향 — 추가는 [docs/10 §3](docs/10-decision-record.md) 원장에 건별 기록 |
| DR-9 | **예산 게이트** — 24시간 상주 제품. 유휴 RSS·바이너리 크기 CI 게이트(수치 미정) |
| DR-10 | **로컬 전용이 기본값** — 클라우드 계정 동기화 안 한다 |
| DR-41 | ★ **최소 처리 원칙**(09-04) — 요청/수행 분리 · 폭주는 큐 대신 **마지막만 덮어쓰기** · 낡으면 취소(세대) / 다 필요하면 제한 개수 점진 · UI 스레드는 기다리지 않는다 |

## 3. 제품 차별점 (조사 결과 — [docs/03](docs/03-competitive-landscape.md))

1. ★ **3-OS 완전 동일 화면** — 경쟁 제품 **0/6**(CopyQ=Qt 룩, EcoPaste·PasteBar=OS WebView)
2. ★ **암호화가 기본값 + 백업까지** — 기본 켜짐은 **0/6**(CopyQ만 옵트인 암호화 있음)
3. ★ **상주 예산 게이트** — 1Clipboard(Electron)가 죽은 이유

> **한 줄** — *"Ditto의 능력 · Maccy의 가벼움 · CopyQ의 이식성을, 프레임워크 없이 우리가 그린 한 벌의 화면으로."*

⚠️ **사용자는 현재 macOS=Maccy, Windows=CopyQ를 실사용 중이다.** 두 제품은 경쟁사가 아니라 **기준선**이고, 성공 판정은 *"이 둘을 지우는가"* 다([docs/03 §5-4](docs/03-competitive-landscape.md)).

## 4. 작업 규약

- **문서·커밋/푸시 규약 SSOT = [docs/16](docs/16-doc-git-conventions.md)** — 4층 문서 체계 · 작성 규칙 8 · 커밋/브랜치/푸시 필수 규칙.
- 기록: 일자 상세 `docs/journal/YYYY-MM-DD.md`(시간 역순) + [DEVLOG](docs/DEVLOG.md) 요약 + [MILESTONES](docs/MILESTONES.md) + [BRANCHES](docs/BRANCHES.md). **한 작업 = 한 트랜잭션 갱신**.
- **큰 단위 = 브랜치, 세부 기능 = 커밋. push는 사용자 명시 요청 시에만.**
- 스테이징은 `git add <파일>`로 내가 고친 것만. **`git add -A`·`git add .` 금지.**
- 🔴 **push 전 `scripts/check-3os.sh`**(3타깃 clippy) · push 뒤 `gh run watch`. cfg 모듈 경로를 손대면 **다른 OS 모듈 깊이**를 같이 본다(win·sni 1단 · mac::hotkey 2단 — 09-05 CI 16회 빨강의 교훈).
- **기능 설계 전 `nexa-beep`·`nexa-dir2` 문서·코드 먼저 확인**(재발명 금지). 이식 커밋에 원본 경로 명기.
- 🔴 **모든 변경에서 상시 점검** — 이 변경이 ① `nexa-beepd`(서버) ② `nbeep-relay` 와이어 ③ beep과 공유하는 규약(도메인 문자열·prologue·타이브레이크)을 건드리는가?
  하나라도 예면 **[docs/22 전달 원장](docs/22-upstream-beep-liaison.md)** 에 기록하고 사용자에게 알린다. ⚠️ beep 저장소 직접 수정은 **승인 대상**(다른 프로젝트).
- `.claude/settings.json`(권한)은 **덮어쓰기 금지, 병합만**.

## 5. 새 세션 오리엔테이션

1. 이 CLAUDE.md + [docs/STATUS.md](docs/STATUS.md) → 2. [DEVLOG](docs/DEVLOG.md) 최상단 + 최신 journal → 3. 할 일 = [docs/TODO.md](docs/TODO.md).

## 6. 다음 단계 (2026-08-25)

> ★ **지금 병목은 코드가 아니라 결정이다.** [docs/10 §2](docs/10-decision-record.md#2-열린-결정-d--색인)에 **열린 결정 29건**이 있고, **11건은 권장안까지 나와 사용자 확정만 남았다**(🔴).

1. ✅ **핵심 결정 확정 완료(08-31)** — D-20→DR-28 · D-9→**DR-38**(기본 켜짐) · D-1→**DR-37**(자체 직렬화) · D-24~29→**DR-39** · D-38~40→**DR-40**.
2. **ADR 승격** — 확정된 큰 결정을 `NN-adr-000N-*.md`로 고정.
3. **05 요구사항 문서**(FR/NFR 확정) + **07 ADR-0001 스택**(예산 수치 포함) 작성.
4. **코드 착수** — 워크스페이스 `Cargo.toml` · `nclip-core` 항목 모델 · `nclip-plat` 클립보드 감시(3-OS) 수직 슬라이스.
   ⚠️ **난이도는 전부 `nclip-plat`에 모인다** — 클립보드 감시 3방식 · 전역 단축키 · **직전 포커스 창 복원 + 키 주입**.
5. `15-dev-methodology.md` · `18-build-and-test.md` 생성(번호 예약됨).
