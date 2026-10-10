# CLAUDE.md — Nexa Clip 프로젝트 컨텍스트 (이식용 메모리)

> 이 파일은 **다른 PC에서 clone 시 즉시 컨텍스트를 공유**하기 위한 휴대용 프로젝트 메모리다.
> **먼저 읽기:** [docs/STATUS.md](docs/STATUS.md)(현황) → [docs/10-decision-record.md](docs/10-decision-record.md)(결정).

## 1. 이 프로젝트는

**Nexa Clip** = **크로스플랫폼 클립보드 매니저**(Windows · macOS · Linux).
**올 러스트 · 단일 바이너리 · 자체 CPU 래스터라이저**로 **3-OS 완전 동일 화면**을 그린다 — Qt·WebView·Electron을 쓰지 않는다.

- 조직: **SosomLab** · 개발자: Sangyong Bae · kiros33@gmail.com
- 저장소: <https://github.com/SosomLab/nexa-clip> · 라이선스: **PolyForm Noncommercial 1.0.0**
- 현 단계: ★ **M2 진행 중 · v0.2.0 배포됨**(10-10 · nexa-ui 형제 저장소 path 의존 첫 배포 · 설정 카드 · 라이선스 · brew ✓ · pkg.sosomlab.com ✓ APT+RPM · **전반 점검 ✓**(콜드 스타트 105ms · 유휴 13MB · 10분 누수 0 · Windows E2E `scripts/win-e2e.ps1` 8/8) · 결함 1 수정 = Win 붙여넣기 settle(ed494f3 · v0.2.0 미포함 → v0.2.1 후보 T-74) · winget 0.1.6 PR #446464·#446465 모더레이터 대기 → 0.1.7~0.2.0 미제출(guard skip) · choco 0.1.5 검수 중(설치본 VirusTotal 6~10건 = 예외 필요) → 0.1.6~0.2.0 미제출) — M1(감시·캡처·암호화 영속·팝업·메인창·설정·트레이·주입 3-OS) 완료 · 동기화(릴레이+LAN 직결·기기 승인·전파) · 리치 렌더 2단(표 격자 · 컬러 이모지 Linux) · 검색(색인·정규식) · 메모리 상주 계층(DR-42) · 배포 파이프라인(brew · winget · choco). ★ Linux 서명 저장소 **pkg.sosomlab.com**(APT · RPM)은 `SosomLab/linux-repo`가 발행한다(릴리스 공개 뒤 `app-released` 신호). 핵심 결정은 **DR-47**(공용 UI = nexa-ui 형제 저장소 path 의존 · 10-10)까지 확정. ★ **10-10 nexa-ui 전환 완료(main 병합)**: nclip-gfx·ctl·conf 사본 삭제 → nexa-ui path 의존 · 설정 카드 개편(그룹 트리·고급·종속 잠금·자모 검색 · beep P2 동일) · 라이선스(nexa-license 어댑터 · 설정 › 정보 · `--license`) · 파일 선택기(nexa-dlg) · Debug 빌드 창 제목 "(Debug)" — 실기 = [21 §15](docs/21-manual-test.md).

### 참조 원천 (재발명 금지 — 설계 전 반드시 확인)

| 원천 | 로컬 경로 | 무엇을 가져오나 |
|---|---|---|
| **`nexa-beep`** | `../nexa-beep` | ★ **기본 틀** — 크레이트 경계 · `plat` 포트 · CPU 래스터라이저(`nbeep-gfx`) · 컨트롤(`nbeep-ctl`) · 클립보드 어댑터(`nbeep-plat/clipboard.rs`) · 트레이 · **릴레이(`nbeep-relay`·`nexa-beepd`)** · 암호화 키 계층(ADR-0005) · 다중 기기 신원(ADR-0007) |
| **`nexa-dir2`** | `../nexa-dir2` | ★ **컨트롤** — `ctl` 17종 · `nexa-gui`(`draw`·`event`·`geom`·`theme`·`edit`·`typeahead`) · **`grid`/`columns`**(정렬·리사이즈) · 가상화 목록 |
| **`nexa-ui`** | `../nexa-ui` | ★ **공용 UI 라이브러리(path 의존 · DR-47 · 10-10)** — nexa-gfx(래스터·텍스트·PNG 글리프) · nexa-ctl(컨트롤·hangul) · nexa-conf(설정 영속) · nexa-font(시스템 글꼴) · nexa-dlg(파일 대화상자) — clip 사본(nclip-gfx·nclip-ctl·nexa-conf)은 10-10 삭제 · **고칠 것은 nexa-ui에서** · CONSUMER-CHANGES clip 열 |
| **`nexa-license`** | `../nexa-license` | 라이선스 검증(path 의존 · ed25519·machine-id·fs · 검증 전용) — 앱 어댑터 `crates/nclip-license`(10-10 · Feature 0 · 비상업 = Free · `--license` CLI · 설정 › 정보 › 라이선스…) |

> ⚠️ **릴레이 서버(`nexa-beepd`)는 clip에 들어 있지 않다** — `nexa-beep` 저장소 `crates/nexa-beepd`에서 **`beepd-v*` 별도 태그**로 배포되는 **별도 실행 파일**이고, 사용자가 **따로 띄워야** 원격 동기화가 된다(공식 `beepd.sosomlab.com:47300` · LAN만 쓰면 None으로 서버 불요). clip은 `nclip-sync/*`에 와이어 사본만 둔다 → [18 §10-2](docs/18-build-and-test.md) · 위키 [릴레이 서버](https://github.com/SosomLab/nexa-clip/wiki/릴레이-서버).

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

- 🔴 **답변은 한글로** — 최종 답변·중간 진행 알림 모두 한국어로 쓴다(코드·명령·식별자·로그 원문은 그대로).
- ★ **협업 세션 운영 기준(사용자 10-04 · [docs/33](docs/33-collab-session-operation.md) · 원본 = nexa-sql docs/102)** — 설계·개발·소스 수정 = **Fable 개발 세션** · 문서·빌드/시험·재시작·CI 감시·자료 분석 = **Opus 협업 세션**(찾기 `ListAgents` · 연락 `SendMessage`). 한 파일은 한 세션만(소스 = 개발 · 문서 = 협업) · **커밋은 개발 세션 한 곳** · 협업 세션은 git 쓰기 금지 · **[P0] 빌드 + Debug 재시작 최우선** · 검증 V0~V3 · 같은 `target/` 동시 빌드 금지 · 상충은 §12 원장에 · 🔴 **두 세션 모두 수신을 늘 열어 둔다**(사용자 10-05 · [33 §9-0](docs/33-collab-session-operation.md)) — 긴 명령은 백그라운드 · 매 차례 도착 메시지 먼저 · 바로 못 하면 "받음·언제" 한 줄 · 쉬기 전 미답 0 · 권한 모드 맞춤. 새 세션은 §0 시작 절차부터 · **다른 저장소로 가져갈 때는 §0-0**(이미 정리돼 있으면 다시 만들지 않고 "운영 방침은 이미 정리된 상태"라고 답한다).
- 🔴 **winget·Chocolatey 제출 내용은 전부 영어로**(사용자 10-04 · [docs/18 §10](docs/18-build-and-test.md) B) — 제출 틀(`packaging/winget/**` · `packaging/choco/**`)은 설명·요약·스크립트 주석·`Write-Host` 문구·YAML 주석까지 영어만 쓴다(한국어 병기 없음). 나머지 저장소는 한국어 그대로 · Homebrew 탭은 대상 아님 · 강제 = `packaging/render-manifests.sh` 게이트. 검수 상태 점검은 배지·피드가 아니라 **패키지 페이지 상태 문구와 검수자 댓글 전문**으로(choco 0.1.5 반려를 6일 놓친 교훈).
- **문서·커밋/푸시 규약 SSOT = [docs/16](docs/16-doc-git-conventions.md)** — 4층 문서 체계 · 작성 규칙 8 · 커밋/브랜치/푸시 필수 규칙.
- 기록: 일자 상세 `docs/journal/YYYY-MM-DD.md`(시간 역순) + [DEVLOG](docs/DEVLOG.md) 요약 + [MILESTONES](docs/MILESTONES.md) + [BRANCHES](docs/BRANCHES.md). **한 작업 = 한 트랜잭션 갱신**.
- **큰 단위 = 브랜치, 세부 기능 = 커밋. push는 사용자 명시 요청 시에만.**
- 스테이징은 `git add <파일>`로 내가 고친 것만. **`git add -A`·`git add .` 금지.**
- 🔴 **push 전 `scripts/check-3os.sh`**(3타깃 clippy) · 그 앞에 **`rustup update stable`**(CI는 고정 없는 stable — 로컬이 낡으면 새 린트를 못 잡는다 · 10-05 CI 빨강의 교훈 · [33 §5-7](docs/33-collab-session-operation.md)) · push 뒤 `gh run watch`. cfg 모듈 경로를 손대면 **다른 OS 모듈 깊이**를 같이 본다(win·sni 1단 · mac::hotkey 2단 — 09-05 CI 16회 빨강의 교훈).
- **기능 설계 전 `nexa-beep`·`nexa-dir2` 문서·코드 먼저 확인**(재발명 금지). 이식 커밋에 원본 경로 명기.
- 🔴 **모든 변경에서 상시 점검** — 이 변경이 ① `nexa-beepd`(서버) ② `nbeep-relay` 와이어 ③ beep과 공유하는 규약(도메인 문자열·prologue·타이브레이크)을 건드리는가?
  하나라도 예면 **[docs/22 전달 원장](docs/22-upstream-beep-liaison.md)** 에 기록하고 사용자에게 알린다. ⚠️ beep 저장소 직접 수정은 **승인 대상**(다른 프로젝트).
- 🔴 **형제 저장소(nexa-ui · nexa-license) 수정 규칙(사용자 10-10)** — ① **수정 전 최신화**가 기본 단계(`git fetch` → `main`이 뒤졌으면 `pull --ff-only` · 작업 브랜치는 `origin/main` 위로 rebase) — 다른 PC가 같은 저장소를 개발 중일 수 있다 ② **기능 단위로 커밋 + push**(작업 브랜치 → 게이트 → `main` ff 병합 → 즉시 push · clip처럼 모아 두지 않는다) ③ push 순서 = **nexa-ui → nexa-license → nexa-clip**(DR-47 · 거꾸로 밀면 clip CI가 옛 형제를 받아 깨진다) · clone은 세 저장소를 나란히([18 §1](docs/18-build-and-test.md)) · 커밋 직전 fetch도 세 저장소 모두.
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
