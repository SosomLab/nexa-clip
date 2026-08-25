# DEVLOG — 날짜별 요약

> 시간 역순. 항목당 1~2줄. **상세는 [journal](journal/)**, 여기는 요약 + 링크.

## 2026-08-25

- **문서 골격 수립** — CLAUDE.md · docs/README · STATUS/DEVLOG/journal/MILESTONES/TODO/BRANCHES · [10 DR](10-decision-record.md) · [16 규약](16-doc-git-conventions.md)(beep 차용). 확정 DR-1~10, 열린 결정 29건 색인화 → [journal 7차](journal/2026-08-25.md)
- **[09 신원·페어링](09-identity-and-pairing.md)** — 사용자 제안(핸들 직접 등록 + PeerId 수동 승인) 평가. 열거 구멍(R-a) 발견 → **핸들+패스프레이즈 PBKDF2 랑데부**로 제약 삼각형 해소 → [journal 6차](journal/2026-08-25.md)
- **[08 자동 전파](08-clipboard-propagation.md)** — 함정 3개(파일 목록 깨짐·자동 덮어쓰기·에코 루프) + **모바일 수신 최하순위 등재**(iOS/Android 자동 캡처 불가 확인) → [journal 5차](journal/2026-08-25.md)
- **[07 기기 랑데부](07-device-rendezvous.md)** — 3단 검증(랑데부/Noise/DeviceList). ★ 실코드에서 **beepd RID 맵이 1:1**임을 확인해 공유 URID 안 기각 → [journal 4차](journal/2026-08-25.md)
- **[06 저장 설계](06-storage-design.md)** — D-1 답: **파일 직렬화 + 3규칙**. 인덱스 270B/항목 → 1만 항목 2.7MB로 메모리 상주 가능 → FTS 불필요 → [journal 3차](journal/2026-08-25.md)
- **[05 다중 기기](05-multi-device-sharing.md)** — 릴레이 재사용 가능(**서버 코드 변경 0**), 단 릴레이만으로는 불가(UserId 선결). 동기화 두 축 확정 → [journal 2차](journal/2026-08-25.md)
- **[03 경쟁 조사](03-competitive-landscape.md) · [04 기능·화면](04-feature-scope-and-screens.md)** — OS별 24종 + 크로스플랫폼 6종. ★ **사실 2건 정정**(CopyQ 암호화 존재 · Maccy 2.x 이미지 지원). 라이선스 beep 동일 구성 → [journal 1차](journal/2026-08-25.md)
