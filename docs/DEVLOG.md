# DEVLOG — 날짜별 요약

> 시간 역순. 항목당 1~2줄. **상세는 [journal](journal/)**, 여기는 요약 + 링크.

## 2026-08-26

- ★ **디자인 토큰**(T-12d3) — 간격·타입·**상태 레이어**·엘리베이션·모션을 `nclip-ctl::tokens`로. **팝업 120ms 상한을 컴파일 타임 assert**로 → [journal 15차](journal/2026-08-26.md)
- ★ **스플리터 글로우** — 1000ms 인 / 220ms 아웃. **세로 균일** 정정(핸들 제거·색만 보간) · 규칙 M-1~M-3 → [25 §3-7](25-design-system.md)
- ★ **[18 빌드·테스트](18-build-and-test.md)** — 기계가 하는 것(18) / 사람이 하는 것([21](21-manual-test.md)) 분리. ★ **테스트가 "왜" 있는가**를 적었다
- ⚠️ **K-1 정정** — 붙여넣기 성공 ≠ 통과. **탈취 실패 = 복원 경로 미실행**. 임시 최상위 창 탈취 추가 · T-9b/T-9b' 분리 → [journal 15차](journal/2026-08-26.md)
- ★ **설정 창 실물 + VS Code식 스플리터**(T-12d2) — 이식 프레임워크가 실제로 돈다(21항목·검색·즉시 적용). ★ **드래그는 이미 있었고 커서·하이라이트만 없었다** → [journal 14차](journal/2026-08-26.md)
- **DR-31** 실패 통보 = **상태 전이 + 로그**(모달 없음) · `nclip-core::diag` 구현 — ★ **원인과 조치를 쌍으로** · 버려진 줄 수 카운트 · **항목 내용은 안 남긴다** → [journal 13차](journal/2026-08-26.md)
- **DR-29** 팬아웃/승인 분리 — 내 기기=**전체 동시**·남=**특정 Peer** · ★ **신뢰해도 기본 수동 승인**(사람별 자동 승인 토글) · **재전파는 수신측 결정** → [journal 12차](journal/2026-08-26.md)
- **DR-30** 📐 **파일 내용 공유 설계** — 붙여넣는 순간 원본 `PeerId`에서 **지연 수신**(OS 가상 파일 규약) · 같은 `UserId` 한정. ★ **dir2 X-42가 받는 쪽 선례**라 함정까지 알려져 있다 → [26](26-file-content-sharing.md)
- ★ **설정 화면 이식 완료**(T-12d) — 프레임워크 **2,000줄 그대로**, `registry()`만 우리 21항목으로 교체. 화면 코드 무수정. 예측(13 §2-3)이 그대로 맞았다 → [journal 11차](journal/2026-08-26.md)
- ★ **알파 렌더링**([23](23-alpha-rendering.md)) — `fill_rect_alpha` + 데모 반투명 패널(`P`). L1(앱 안) 구현 / L2(창 투명) 분리
- ★ **[24 참조 설정 연구](24-reference-settings-study.md)** — CopyQ 10장 전수. **내 판단 3건이 뒤집혔다**(창 투명·트레이 이미지·활성화 후 동작)
- ★ **[25 디자인 시스템](25-design-system.md)** — Material 골격 + macOS 마감. beep 격차 진단 = **빠진 5개 층**(상태 레이어·엘리베이션·간격·타입·모션)
- **beep 반영**(승인) — `docs/44` 신설 + `rid_for` 주석. ★ **서버 재배포 불필요**를 실코드로 확인
- **DR-24~28** 확정 · **D-20 닫힘**(내 기기 = 승인 없이 즉시 · 남 = 승인) → [journal 11차](journal/2026-08-26.md)
- **DR-23 앱 격리** — 릴레이 탐색은 같은 앱끼리만. **RID 도메인 분리 + Noise prologue**로 ★ 검사가 아니라 **구조**로 강제(서버 변경 0). 브리지는 나중에 옵트인 → [journal 10차](journal/2026-08-26.md)
- 🔴 **[22 beep 전달 원장](22-upstream-beep-liaison.md)** — 서버·와이어 변경 연락 창구. **모든 변경에서 점검하는 체크리스트**를 CLAUDE.md·메모리에 등록. 현재 미전달 3건 → [journal 10차](journal/2026-08-26.md)
- ★ **창·렌더 데모**(T-12b2) — winit/softbuffer + CPU 래스터라이저로 **S1 팝업 레이아웃 실제 렌더**. 보기 3모드·테마 전환. 폰트 mmap 이식(맑은 고딕 확인). 외부 의존 3개(`memmap2`·`winit`·`softbuffer`) 원장 기록 → [journal 9차](journal/2026-08-26.md)
- **DR-22** 클립보드 **보내기** — 제안 → 승인 → 등록. **단일 요청 = 단일 승인** · 수신자 신뢰 목록이 관문 · 미등록은 안내 후 중지 → [journal 9차](journal/2026-08-26.md)
- ★ **K-1 스파이크**(T-9b) — 포커스 복원 + 키 주입. Windows ✅(`AttachThreadInput` 우회 · `INPUT` 40B 컴파일 assert) · macOS 🚧 구현/미검증(objc 런타임 직접 호출) · Wayland ✕. `PasteCapability`를 Full/**NeedsPermission**/ClipboardOnly 셋으로 → [journal 8차](journal/2026-08-26.md)
- ★ **[21 실기 점검표](21-manual-test.md)** — OS별 검증 현황·절차·증상 기록. **⏳(점검 요청)** 을 ✅과 구분 → [journal 8차](journal/2026-08-26.md)
- **DR-19**(동기화=핵심 기능) · **DR-20**(보안 = 네트워크 ≫ 로컬 · D-9 닫힘) · **DR-21**(i18n 4언어) 확정 → [journal 8차](journal/2026-08-26.md)
- ★ **`nclip-*` 명칭 통일(포크)** — vendor 층위·U-1 규율 폐지. 기능 변경 0, **162 테스트 그대로**. DR-15 폐기 → **DR-17**(포크 흡수) · **DR-18**(다음 프로젝트는 라이브러리 선행) → [journal 7차](journal/2026-08-26.md)
- ★ **코드 착수** — 워크스페이스 8 크레이트 · vendor 무수정 복사(`nbeep-gfx`·`nbeep-ctl`) · `nclip-core`(i18n·항목/표현 모델·포트) · `nclip-plat::watch` 게이트 · `ViewMode` · CI(3-OS + **vendor 무결성**). **162 테스트 통과** · clippy `-D warnings` 클린 → [journal 6차](journal/2026-08-26.md)
- ★ **아이콘** — 계열 골격(라운드 스퀘어·그라디언트·흰 전경) 유지 + **클립보드 + 히스토리 스택** 모티프 · 청록 `#22C3D6→#0B7FA6`. SVG 정본 + 1024/256/64 PNG + ICO → [journal 6차](journal/2026-08-26.md)
- **DR-15**(vendor 원본명 유지) · **DR-16**(i18n core 동형) 확정 · D-8/35/37 닫힘 → [journal 6차](journal/2026-08-26.md)
- **[20 구현 설계서](20-implementation-spec.md)** — 화면 레이아웃 개괄 · 기능 목록 · ★ **각 기능의 구현 방법**(감시 3-OS · 다중 표현 · 전역 단축키 · 포커스 복원/키 주입 · 저장 · 검색 · 가변높이 가상화 · 제한 리치 렌더러 · 트레이 · 민감 차단) · 설정 구성 · 크레이트 배치 · **수직 슬라이스 11단계** → [journal](journal/2026-08-26.md)
- **[17 참조 UI 해부](17-reference-ui-teardown.md) · [14 설정 레지스트리](14-settings-registry.md)** — Maccy/CopyQ 실물 기반. ★ **번호 단축키 설계 오류**·**Maccy 파일 캡처 사실 오류** 정정 → [journal](journal/2026-08-26.md)
- **DR-13**(화면 캡처 범위 밖) · **DR-14**(세로 밀도 우선 — 좌측 세로 툴바) 확정 → [journal](journal/2026-08-26.md)

## 2026-08-25

- **[13 beep UI 재사용 계획](13-ui-reuse-from-beep.md)** — 실코드 검증: `nbeep-ctl`은 도메인 의존 0(11,214 LOC 무수정 이식) · 설정 화면은 `registry()`만 교체(3,510) · `MenuBar`/`Toolbar`/`TreeGrid` 기존재. **재사용 ≈21,600 LOC** · 규율 U-1(복사본 불변) → [journal 9차](journal/2026-08-25.md)
- **[00 비전](00-vision.md) · [02 로드맵](02-roadmap.md)** — 핵심 편의성 8(측정 가능 기준) · 2026 흐름 11건 · **기능별 진행 목표 매트릭스**(10영역 × M1~M4) + 수용 기준 · 리스크 K-1~6 → [journal 8차](journal/2026-08-25.md)
- **[12 클립보드 포맷](12-clipboard-formats.md)** — Word/PPT Rich Text는 HTML/RTF만으론 왕복이 깨진다(PPT 도형→그림). **원본 포맷 이름째 보존**으로 FR-C-5·C-6 P0 승격 · 붙여넣기 4모드 → [journal 8차](journal/2026-08-25.md)
- **화면 구성 확정(DR-12)** — 트레이 우클릭 **최근 8개** · 메인창 = 메뉴바+툴바+검색바(정규식)+목록 · **보기 3모드**(일반/간략/한 줄) → [journal 8차](journal/2026-08-25.md)
- **문서 골격 수립** — CLAUDE.md · docs/README · STATUS/DEVLOG/journal/MILESTONES/TODO/BRANCHES · [10 DR](10-decision-record.md) · [16 규약](16-doc-git-conventions.md)(beep 차용). 확정 DR-1~10, 열린 결정 29건 색인화 → [journal 7차](journal/2026-08-25.md)
- **[09 신원·페어링](09-identity-and-pairing.md)** — 사용자 제안(핸들 직접 등록 + PeerId 수동 승인) 평가. 열거 구멍(R-a) 발견 → **핸들+패스프레이즈 PBKDF2 랑데부**로 제약 삼각형 해소 → [journal 6차](journal/2026-08-25.md)
- **[08 자동 전파](08-clipboard-propagation.md)** — 함정 3개(파일 목록 깨짐·자동 덮어쓰기·에코 루프) + **모바일 수신 최하순위 등재**(iOS/Android 자동 캡처 불가 확인) → [journal 5차](journal/2026-08-25.md)
- **[07 기기 랑데부](07-device-rendezvous.md)** — 3단 검증(랑데부/Noise/DeviceList). ★ 실코드에서 **beepd RID 맵이 1:1**임을 확인해 공유 URID 안 기각 → [journal 4차](journal/2026-08-25.md)
- **[06 저장 설계](06-storage-design.md)** — D-1 답: **파일 직렬화 + 3규칙**. 인덱스 270B/항목 → 1만 항목 2.7MB로 메모리 상주 가능 → FTS 불필요 → [journal 3차](journal/2026-08-25.md)
- **[05 다중 기기](05-multi-device-sharing.md)** — 릴레이 재사용 가능(**서버 코드 변경 0**), 단 릴레이만으로는 불가(UserId 선결). 동기화 두 축 확정 → [journal 2차](journal/2026-08-25.md)
- **[03 경쟁 조사](03-competitive-landscape.md) · [04 기능·화면](04-feature-scope-and-screens.md)** — OS별 24종 + 크로스플랫폼 6종. ★ **사실 2건 정정**(CopyQ 암호화 존재 · Maccy 2.x 이미지 지원). 라이선스 beep 동일 구성 → [journal 1차](journal/2026-08-25.md)
