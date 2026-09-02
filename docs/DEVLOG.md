# DEVLOG — 날짜별 요약

> 시간 역순. 항목당 1~2줄. **상세는 [journal](journal/)**, 여기는 요약 + 링크.

## 2026-09-02 (1·2차) — 검색 완전판 · 최상위 고정 · 동적 언어 · 실기 M 반영

- 팝업 검색 = 메인과 동일(캐럿 깜밖임·×·우클릭 메뉴·IME) · 최상위 고정 · TextBox word-wrap(Alt+Z)·더블/트리플 클릭 · ★ i18n 29키 + **동적 언어**(app.lang 부팅 미적용 결함 발견) → [journal 1차](journal/2026-09-02.md)
- 실기 P 회신(4차) — 휠 스냅백(paint의 ensure_visible 제거) · ★ 평문 순위 선택(`plain_of` — CF_HTML 헤더 노출 해소) · Object 이미지 미리보기+텍스트 폴백 → [journal 4차](journal/2026-09-02.md)
- ★ K4 미리보기 패널(3차) — 툴바 눈 토글 · 기본 접힘 · 텍스트 전문(wrap·휠) / 이미지 원본(지연 디코드 1600) · `ui.preview_open` 영속 → [journal 3차](journal/2026-09-02.md)
- 실기 M 회신: ★ 메인 ×(지우기) 후 빈 목록 잔류 수정(MouseDown 변화 감지) · 편집 시트 우상단 줄 바꿈 스위치(M4 · Alt+Z 동기) → [journal 2차](journal/2026-09-02.md)

## 2026-09-01 (6차) — 단기 개발 일괄 7건

- T-13 예산(무제한+500MB·휘발 토큰 제외) · T-15b 4모드 키 · ★ T-17 한글 조합 검색(이식+IME) · S2(우클릭 메뉴·보기 3모드·★ 평문화 편집) · T-14d CF_HTML 정제 1단 · T-14f/T-21 판정. 결정 4건 선확정(암호화 보류 포함). 398 테스트 → [journal 6차](journal/2026-09-01.md)

## 2026-09-01 (4·5차) — 복원 43ms · 아이콘·폴백·트레이 설정

- ★ AEAD 단형화 착지 발견(호출측 dev opt 3) → 복원 5.7s→43ms · Material 20px 아이콘 확정(시안 아티팩트) · 툴팁 가림/확대/⇧두부 · 창 위치 기억 · `ui.font_family` + ★ 글리프 폴백 체인(두부 원천 차단) · 트레이 "설정" 메뉴 → [journal](journal/2026-09-01.md)

## 2026-09-01 (2·3차) — ★ S2 메인창 1단

- T-18b0: [28](28-main-window.md) 설계 + `main_win.rs`(검색·세로 툴바·핀 구획·삭제/핀 영속) · 트레이 좌클릭 = 메인창. 실기 후속 7건(클램프·창 앞으로·키/휠 라우팅·MouseUp·아이콘) → [journal](journal/2026-09-01.md)

## 2026-09-01 (1차) — 실기 후속 셋

- ★ "종료 시 기록 사라짐" = 소실 아님(진단 덤프 전수 · debug 암호 미최적화 시작 블록 → dev opt 3) · 팝업 모니터 클램프 · ★ ListEditor 신설(차단 목록 ListBox+인라인 편집 · 값 `;` 불변). 374 테스트 → [journal](journal/2026-09-01.md)

## 2026-08-31 (4차) — ★ T-16 1단: 암호화 영속

- `nclip-store` — beep sealed 이식 + 이벤트 로그/압축 + 내용 주소 blob(중복 제거) + 키 래핑 한 겹. 이력 id·★핀 기초·복원 배선 · `sec.clear_on_quit` wipe. 스모크 "2개 복원" ✓ · 368 테스트 → [journal 4차](journal/2026-08-31.md)

## 2026-08-31 (3차) — 핵심 결정 확정: DR-37~40

- T-1~T-4b 소화 — 저장 구조(DR-37) · ★ at-rest 암호화 기본 켜짐(DR-38 · DR-20 로컬 조항 대체) · 신원·페어링(DR-39) · UI 셋(DR-40). T-16 착수 가능 → [journal 3차](journal/2026-08-31.md)

## 2026-08-31 (2차) — Windows 실기: 결함 둘 + "서식 유실"은 Word 설정

- 창 아이콘 부착(`icon.rs`) · ★ ⇧Enter 재열림(주입 Ctrl+Shift+V = 자기 단축키 + 물리 Shift 잔류 → Ctrl+V 통일·수식 키 중화) — 둘 다 실기 통과. ★ 빨강 유실은 왕복 바이트 동일 + Word COM 실험으로 **Word "서식 병합" 설정** 판정(결함 아님 · Win+V도 동일). 왕복 진단 `clip_roundtrip` 신설. 350 테스트 → [journal 2차](journal/2026-08-31.md)

## 2026-08-31 (1차) — Windows 자리 최신화: main이 red였다

- Linux 자리 38커밋 ff → **Windows·mac test cfg 빌드 실패**(`has_data_control` linux 한정 · `launcher_content` dead_code). `not(linux)` 짝 + `.desktop` 계약 테스트로 복구. ★ CI는 이미 잡고 있었다(run 33262111549 failure 방치) — 병합 전 **크로스 `clippy --all-targets` 3타겟**으로 예방. 350 테스트 → [journal](journal/2026-08-31.md)

## 2026-08-30 (3차) — 실기 라운드 2

- **주입 순서**(팝업 파괴 flush 뒤 다음 바퀴 — "첫 번만 붙음" 해소 · 사용자 확인 텍스트·이미지 ✅) · ★ **닫기=트레이 기본 켜짐**(사용자 확정 · Quit만 종료) · **부분 게시 에코 = 원본 승격**(팝업 중복) · ★ **`ui.theme=system` 실제 추종**(beep theme.rs 이식 · 포털 실측). 352 테스트 → [journal 3차](journal/2026-08-30.md)

## 2026-08-30 (2차) — 사용자 실기 셋 수정

- ★ **Wayland 키 주입 = 포털 RemoteDesktop**(ei-portal Xwayland에선 XTest가 앱까지 못 간다 — 1차 가정 정정) · ★ **`wl-paste` 폴링이 포커스를 뺏던 것**(GNOME data-control 부재) → 레지스트리 사실 판정 후 XWayland `xclip` · 타이틀바 두부 → ASCII · Dock 톱니바퀴 → 런처 `.desktop`+아이콘. 349 테스트 → [journal 2차](journal/2026-08-30.md)

## 2026-08-30 — ★ Linux 트레이 · 포털 단축키 · K-1 Linux · 클립보드 쓰기

- ★ **T-12e Linux** — beep SNI 이식 + 최근 항목/알림 확장 · `wlactivate`(셸 토큰 → 진짜 포커스) · app_id. **D-Bus 실측**: 등록·메뉴·재적재(`wl-paste`)·Quit 전부 ✓.
- ★ **T-15 Linux = xdg 포털 GlobalShortcuts**(`hotkey_linux.rs` 신설) — 세션 ✓ · 등록은 사용자 대화창 대기.
- ★ **T-9b K-1 Linux** — x11rb XTest. X11 = 포커스 기억/복원 · Wayland = 컴포지터 반환 + XWayland XTest(`DISPLAY` 없으면 정직 강등). **Xvfb 자동 왕복 ✓**.
- 실기가 잡음: **Linux 클립보드 쓰기 미이식** → wl-copy/xclip 1단(표현 1개). 347 테스트 · 의존 +5(Linux 한정). 상세 → [journal](journal/2026-08-30.md)

## 2026-08-29 (4차) — Linux 남은 확인 사항: X11 · Weston · 프로토콜 실측

- ★ **X11(`xclip`) 경로 실기 통과** — X11 세션 없이 **XWayland**(`WAYLAND_DISPLAY` 제거)와 **순수 Xvfb**(루트 없이) 둘 다 7/7. 하네스 `NCLIP_E2E_X11=1`/`NCLIP_E2E_XVFB=1` 모드.
- ★ **Weston 14.0.2 nested**에서 `wayland-wl-paste` 정판정 · `wayland-info` 실측: **Mutter 50.1·Weston 모두 data-control(wlr·ext) 없음** — 1단이 도구 파이프인 근거가 측정값이 됐다.
- 결함 하나 — 빈 클립보드에서 `peek`의 *"다른 앱이 잡고 있다"* 는 Linux에서 거짓(Windows 사정) → OS별 사실 안내. 남은 것: Nautilus 실제 표현(사람 `Ctrl+C`) · KWin/Sway. 상세 → [journal 4차](journal/2026-08-29.md)

## 2026-08-29 (3차) — Linux 실기 환경 + 결함 여섯

- ★ **sudo 없이 Linux 개발환경 구축**(rustup + 로컬 deb 프리픽스) — `cc` 없이는 크로스 check도 안 된다는 것을 실측. [docs/18 §1](18-build-and-test.md) 정정.
- ★ **Linux 자동 실기 하네스**(8유형) — `watch.rs` git 바이너리 · 빈 클립보드 영구 활동 주기 · 상한 사후 적용 · **파일 잘라내기 유실** · `MissingTool` 오안내 · ★ **부분 스냅숏**(같은 복사 두 항목 + 오분류) 여섯 수정.
- 📄 [docs/18 §9](18-build-and-test.md) — Linux 환경 **배포판별 차이 포함**. 상세 → [journal](journal/2026-08-29.md).

## 2026-08-29

- ★ **mac 실기 결함 둘 수정** — ① **한글 파일명 자모 분해**(macOS NFD) → `compose_hangul_nfd`(한글 한정 NFC · 표시만 · 3-OS 멱등) ② **출처 앱 박제**(`NSWorkspace`는 런루프 없이 안 갱신 — 제외 앱 게이트가 오판) → `CGWindowListCopyWindowInfo`. 실기 ✅ `출처: Finder`·`한글검증_v1.0.pdf`. 관찰: PPT ole.source 토큰이 복사마다 달라 이력 승격 판정 저해 소지(T-13 후속) → [journal 2차](journal/2026-08-29.md)
- ★ **T-14e macOS 감시 + Linux 감시 1단** — 3-OS가 처음으로 다 잡는다. mac = `changeCount` 적응형 폴링(objc 직접 호출 · 결함 ⑫ 교훈 선반영 · nspasteboard 표식 3종 · ★ **Finder 참조 URL을 경로로**) — **유형별 실기 통과**(한글 텍스트·RTF·PNG 치수·Finder ⌘C 파일). Linux = `wl-paste`/`xclip` 파이프 + 내용 지문(`MissingTool` 사유 신설 · 실기는 환경 대기). 334 테스트 · 3-타깃 클린 → [journal 1차](journal/2026-08-29.md)

## 2026-08-28

- **Ctrl+C = 정상 종료** — `STATUS_CONTROL_C_EXIT` 오류 오인 해소(`SetConsoleCtrlHandler` → 셸 종료 경로 · 설정 flush · exit 0) → [journal 10차](journal/2026-08-28.md)
- ★ **아침 실기 피드백 4건 반영** — ① 단축키 `v` 유출(150ms 유예+Ctrl 가드) ② **이미지 썸네일 + `ui.image_preview` 설정**(DIB 순수 · PNG는 격리 워커 첫 소비) ③ 중복 복사 시 팝업 커서 맨 위로 ④ **마우스 배선**(클릭=선택+붙여넣기 · 휠 스크롤). 트레이·설정·숨기기·팝업 실기 정상 확인 → [journal 9차](journal/2026-08-28.md)
- ★ **T-19 이미지 격리 디코더** — `nclip-imgdec` 크레이트째 이식(파서는 워커에만 · 파싱 전 권한 강등 — ★ **lockdown 실기 확인**) + `plat::imgdec` 어댑터(부모 kill 시간 상한 · NIMG 재검증). E2E 픽셀 왕복 비트 동일. 셸 설정 즉시 반영도 → [journal 8차](journal/2026-08-28.md)
- ★ **S1 퀵 팝업 1단(T-18) + 전역 단축키(T-15 Win)** — `Ctrl+Shift+V` → 포커스 기억 → 커서 팝업(실데이터·검색) → Enter 원본/⇧Enter 평문 → 재적재 + **복원·주입**. ⚠️ 생성 직후 `Focused(false)` 레이스 발견 → `was_focused` 가드. 잠금 화면 한계로 열림/닫힘까지 자동 검증 → [journal 7차](journal/2026-08-28.md)
- ★ **제품 루프 1차** — `nclip-core::history`(재복사 = 승격 · D-80 교체 · 상한) + 트레이 우클릭 **최근 N개 메뉴** + 클릭 = **재적재**(`clipboard::set_reps` · beep ClipGuard 힙 오염 예방 이식 · 왕복 테스트). 게이트 공용화 — 막힌 건 이력에도 없음 → [journal 6차](journal/2026-08-28.md)
- ★ **자동 시작 + 닫을 때 트레이로**(사용자 요청) — beep `autostart.rs` 3-OS 이식(토글 즉시 적용 · 시작마다 경로 재동기화 · ★ 외부 삭제 존중) + `ui.close_to_tray`(기본 꺼짐 · beep 확정 준용). `tray`가 **상주 셸**로 승격(winit 한 루프 + 프록시 · 열기 = 설정 창 · 닫기 = 종료/숨기). E2E: HKCU Run 등록/해제 실측 → [journal 5차](journal/2026-08-28.md)
- ★ **T-12e 트레이 상주(Windows)** — beep `tray.rs` 이식 · `nexa-clip tray`(아이콘은 코드로 · 감시 통합 = 수집 수 툴팁 반영). ★ 이식이 `clashing_extern_declarations` 7건을 드러내 **`win32` 공용 선언 모듈로 통합**(paste·watch_win·tray). mac/Linux는 의존 결정과 실기 환경에서 후속 → [journal 4차](journal/2026-08-28.md)
- ★ **T-14c 1단 — `nclip-core::img`** — 치수(PNG IHDR·DIB 머리글) · `dib_to_rgba`(beep 이식) · **박스 평균 축소**. ★ 압축 디코드는 본체에 안 둔다(beep `imgdec` 격리 워커 선례 → T-19). watch가 `[이미지] W×H`를 보여 준다 → [journal 3차](journal/2026-08-28.md)
- ★ **차단 목록을 설정으로**(사용자 요청) — `sec.conceal_urls`(URL 접두 · 기본=코어 목록 · 테스트가 동기화 강제) · `sec.exclude_apps`(FR-S-2 제외 앱 첫 구현 · 완전 일치). ★ **사용자가 목록의 주인** — 뺀 기본 항목은 통과(E2E 박제) → [journal 2차](journal/2026-08-28.md)
- **DR-36 실기 종결** — Chrome ✅(3건 마스킹) · ⚠️ **Edge는 실경로가 `edge://settings/autofill/passwords`**(wallet 아님 — 주소창으로 확정) → 목록 추가 후 ✅ **Edge도 차단 확인** — T-21b 통과 → [journal](journal/2026-08-28.md)

## 2026-08-27

- ★ **T-14g 캡처 디바운스**(D-80 닫힘) — `coalesces`(같은 앱 + 동일 재게시 ‖ 부분⊂완본만) + 500ms 수신 창. ⚠️ 다른 내용은 절대 안 합침. 실기 E9: 탐색기 복사 1회 = 1항목 → [journal 11차](journal/2026-08-27.md)
- ★ **DR-36 브라우저 암호 차단 옵션**(사용자 확정 · **기본 꺼짐**) — `sec.conceal_browser_pw` + `is_password_manager_url`(출처 URL 휴리스틱 · `edge://` 스킴은 웹이 흉내 못 냄). 모의 E2E 통과 · ~~D-79~~ 닫힘 → [journal 10차](journal/2026-08-27.md)
- ✅ **T-14b''' 통과**(사용자 확인: *"이제 정확하게 잡히고 있어"*) — Windows 감시 실기 안정. 감시 결함 ⑫개 전부 실기 발견 → 실기 종결
- ★ **하이젠버그 해부 — 진짜 원인은 빈 스냅숏의 `LAST_SEQ` 오염** — 탐색기 비우기/채우기 틈을 읽으면 표현 0개 + 최종 일련번호 · 채우기는 번호를 안 올림 → dup·하트비트 오판으로 영영 유실. diag는 출력 지연으로 우연히 회피. **내용 없는 스냅숏 = 미처리**로 고침 · E8 검증 4/4 → [journal 9차](journal/2026-08-27.md)
- ✅ **하트비트 실기 통과** — 사용자 8건 전부 포착 · `하트비트 — 놓친 변화 감지` **2건이 로그에**(유실 실재 + 안전망 동작). 후속 → **D-80**(연속 재게시 중복 · 플러시 중간 스냅숏 디바운스) → [journal 8차](journal/2026-08-27.md)
- ★ **`WM_CLIPBOARDUPDATE`가 안 오는 일이 실제로 있다** — 탐색기 복사 간헐 유실을 **직접 재현**(클립보드는 바뀜 · 이벤트만 없음). ★ **일련번호 하트비트**(2초 · 클립보드 안 여는 호출 1회)로 안전망 + `NEXA_CLIP_DIAG=1` 진단. 실기 왕복 6회 전부 잡힘 → [journal 7차](journal/2026-08-27.md)
- ★ **Edge 암호 관리자는 표식을 안 붙인다**(실기) — 암호 복사가 평문 `Text`로 잡힘. FR-S-1(표식 존중)만으로는 부족 → **D-79**(제외 앱 + `Chromium internal source URL` 출처 판정) → [journal 6차](journal/2026-08-27.md)
- ★ **3차 훑기(Word·탐색기)가 결함 셋을 또** — ⑧ 잘라내기가 `Object`(**`CF_HDROP`이 안 온다** → `Shell IDList Array`를 파일 포맷으로) ⑨ ★ **복사가 아예 안 잡힘**(열기 실패를 조용히 버림 + 업데이트 병합 → **타이머 재시도**) ⑩ **민감 표식 무시**(`concealed`를 채우고 안 읽음 → 마스킹). Word ✅ · [19]·[20] 박제 → [journal 5차](journal/2026-08-27.md)
- **창 점검 4건**(사용자 실기) — 포커스 복원 재검증 ✅ · 설정 쓰기 ✅ · 숫자 표시 ✅ · 데모 ⚠️ **두부** = 이모지가 맑은 고딕에 없음(폴백 없음) → KS X 1001 도형 문자로 교체. ★ 제품 아이콘은 글리프로 안 그린다 → [journal 5차](journal/2026-08-27.md)
- ★ **실기 18건이 결함 셋을 더** 잡았다 — ⑤ **PPT 도형이 `Image`** 로(→ ★ **`ClipKind::Object` 신설**) ⑥ `JFIF`·`GIF`·SVG가 **벤더로 세어짐** ⑦ ★ **표현 0개 항목**이 생김(`has_content`). ★ 18건을 **회귀 테스트로 박제** → [journal 4차](journal/2026-08-27.md)
- ★ **Windows 클립보드 감시**(T-14b) — 메시지 전용 창 + 전용 스레드 · `nexa-clip watch` 신설. **제품이 처음으로 동작한다** → [journal 3차](journal/2026-08-27.md)
- ★ **첫 실행에서 버그 넷** — ① `CF_UNICODETEXT`가 **UTF-16LE**(한글이 사라졌다) ② `DataObject`가 텍스트를 리치로 ③ **`CF_OEMTEXT` 누락** ④ *"벤더면 무조건 리치"* 가 그림까지 리치로. ★ **전부 오류를 안 내고 조용히 틀렸다** → [27 §8-1](27-capture-cases.md)
- 설계 정정 — `ClipSnapshot`이 **`RawRep`**(감시는 `blob_id`를 만들 수 없다) · `capture`가 `RepInfo` 제네릭 · `Captured.keep` · `PreviewMissing::ThumbMissing`
- ⚠️ **macOS Retina 배율** — 레이아웃은 2배인데 글자만 1배였다. `RasterCtx::new`에 **배율을 필수 인자로** 승격해 **구조로** 막았다 · ✅ macOS K-1 실기 통과(iTerm 권한) → [journal 2차](journal/2026-08-27.md)
- ⚠️ **macOS 빌드 복구** — K-1 `appkit` 헬퍼가 `forbid(unsafe_op_in_unsafe_fn)` 아래서 **컴파일된 적이 없었다**(E0133 × 13). **T-12c 설정 읽기 mac ✅** → [journal 1차](journal/2026-08-27.md)

## 2026-08-26

- ★ **캡처 파이프라인**(T-14a) — [27](27-capture-cases.md)의 규칙이 `nclip-core::capture`가 됐다(순수 함수 · 실제 클립보드 없이 전수 테스트). ★ **벤더를 목록으로 알아보지 않는다**(아는 표준이 아니면 벤더) · ⚠️ 그 함정은 **곁다리**(`CF_LOCALE` 하나로 텍스트가 리치가 된다) · `Preview::None`에 **사유**를 담는다 → [journal 18차](journal/2026-08-26.md)
- ⚠️ **점검 화면이 거짓말을 하고 있었다** — `default view`가 저장값이 아니라 `ViewMode::default()`였다. 저장본을 읽도록 고치면서 **영속을 터미널에서 검증**할 수 있게 됐다 → [journal 17차](journal/2026-08-26.md)
- ★ **개수 설정을 숫자로**(사용자 확정) — `SettingKind::Number` 신설. *"보통"* 이 몇 개인지 알려면 설명을 읽어야 하지만 **`1000`은 그 자체로 답**이다. `store.max_items`(10~100000) · `ui.tray_recent_n`(3~20) · 범위 밖은 경고+원복 → [14](14-settings-registry.md)
- ★ **붙여넣기 모드 4개**([DR-35](10-decision-record.md)) — 원본/평문/**객체로**/**경로만**. **설정이 아니라 단축키**(붙여넣기 전에 앱을 예상해 설정을 바꾸는 건 순서가 거꾸로다). ★ 넷 다 *"고르는 게 아니라 빼는"* 같은 기계 · `PasteAs::applicable`이 단일 원천 → [journal 16차](journal/2026-08-26.md)
- ★ **설정 영속**(T-12c2) — `nexa-conf` **이름 유지**([DR-32](10-decision-record.md) · 공용 크레이트 추출의 첫 후보). 지연 저장 · 종료 flush · **미지 키 보존** · 포터블 우선([DR-33](10-decision-record.md)) → [journal 16차](journal/2026-08-26.md)
- ★ **hover 페이드**([DR-34](10-decision-record.md)) — 1000ms 인 / 220ms 아웃. ★ **설정 화면에 hover가 아예 없었다** → `Fade`·`HoverFade`·`hover_alpha` 공용 부품 신설, 트리·버튼·콤보 적용 → [journal 16차](journal/2026-08-26.md)
- ★ **[27 케이스별 캡처·표시](27-capture-cases.md)** — ⚠️ **PPT 도형에도 CF_DIB가 있다**(종류 판정 순서가 전부) · ★ **미리보기는 Office가 넣어 준 비트맵으로**(GVML을 안 읽어도 된다) · `Preview` 분리로 정제를 한 지점에
- ★ **[26 §4-4·4-5](26-file-content-sharing.md)** — 원격 파일 **상한은 제안할 때** 검사(실패할 약속을 안 한다) · 앱 탐지 없음
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
