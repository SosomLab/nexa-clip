# DEVLOG — 날짜별 요약

> 시간 역순. 항목당 1~2줄. **상세는 [journal](journal/)**, 여기는 요약 + 링크.

## 2026-08-26

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
