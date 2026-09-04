# 14 · 설정 레지스트리 명세 — Maccy 실측 기반

> **근거**: 사용자가 제공한 **Maccy 설정 화면 캡처 5장**(General · Storage · Appearance · Ignore · Advanced · ※ Pins 탭은 미캡처) — 2026-08-26.
> **목적**: [13 §2-3](13-ui-reuse-from-beep.md#2-3--설정-화면--프레임워크-그대로-데이터만-교체)에서 *"`registry()`만 교체하면 설정 화면 3,510 LOC가 그대로 산다"* 고 했다.
> **이 문서가 그 교체할 데이터의 명세다.** 착수 순서 [13 §6](13-ui-reuse-from-beep.md#6-착수-순서-빠른-경로) 3단계의 입력.
>
> ★ **사용자가 매일 쓰는 제품의 검증된 설정 구성**이므로, 발명하지 않고 **먼저 베끼고 그 위에 우리 것을 더한다.**

---

## 1. Maccy 설정 전수 (캡처 실측)

### 1-1. General

| 항목 | 형태 | 값(캡처) |
|---|---|---|
| Launch at login | 체크 | ✅ on |
| Check for updates automatically | 체크 + **`Check now`** 버튼 | off |
| **Open** | 단축키 필드 + ✕ | `⇧⌘C` |
| **Pin** | 단축키 필드 + ✕ | `⌥P` |
| **Delete** | 단축키 필드 + ✕ | `⌥⌫` |
| **Preview** | 단축키 필드 + ✕ | `^Space` |
| **Search** | 택일 | `Exact` |
| Behavior: **Paste automatically** | 체크 | ✅ on |
| Behavior: **Paste without formatting** | 체크 | off |
| (안내문) | 정적 텍스트 | ★ 아래 [§2-1](#2-1--수식-키가-설정을-덮어쓴다) |
| Notifications and sounds | 링크 + `?` | OS 설정으로 |

### 1-2. Storage

| 항목 | 형태 | 값 |
|---|---|---|
| **Save: Files / Images / Text** | 체크 3개 | ✅ 전부 on |
| **Size** | 숫자 + 스핀 + ★ **실사용량 표시** | `200` · **189.3 MB** |
| **Sort by** | 택일 | `Time of last copy` |

### 1-3. Appearance

| 항목 | 형태 | 값 |
|---|---|---|
| **Popup at** | 택일 | `Cursor` |
| **Pin to** | 택일 | `Top` |
| Image height | 숫자+스핀 | `40` |
| Open Preview automatically | 체크 | ✅ |
| Preview delay | 숫자+스핀(ms) | `1500` |
| **Highlight matches** | 택일 | `Bold` |
| Show special symbols | 체크 | ✅ |
| Show menu icon | 체크 + **아이콘 택일** | ✅ |
| Show recent copy next to menu icon | 체크 | off |
| Show search field | 체크 + 택일 | ✅ `Always` |
| Show title before search field | 체크 | ✅ |
| Show application icons | 체크 | off |
| Show hex color swatch | 체크 | ✅ |
| Show footer | 체크 | ✅ |

### 1-4. Ignore — ★ **3중 구조**

| 하위 탭 | 내용 |
|---|---|
| **Applications** | 목록 + `+`/`−` · ★ **"Ignore all applications except listed"**(허용 목록으로 반전) |
| **Pasteboard types** | 클립보드 타입 단위 무시 |
| **Regular expressions** | 정규식 패턴 무시 |

> ⚠️ **Maccy 자신의 안내문**: *"앱 기반 무시는 **bullet-proof가 아니니** 가능하면 **pasteboard types를 쓰라**."*

### 1-5. Advanced

| 항목 | 내용 |
|---|---|
| **Turn off** | 새 복사를 **일시 무시**. `defaults write org.p0deje.Maccy ignoreEvents true/false` |
| (조작) | ★ 메뉴 아이콘을 `⌥` 클릭 = 토글 · **`⌥⇧` 클릭 = 다음 복사 1건만 무시** |
| **Clear history on quit** | 체크 |
| **Clear the system clipboard too** | 체크 |

---

## 2. ★ 배울 점 — 설계 통찰 6개

### 2-1. ★ 수식 키가 설정을 덮어쓴다

General 안내문 그대로:

> *Select with `⌥` pressed to **copy** item.*
> *Select with `⌘` pressed to **copy and paste** item.*
> *Select with `⇧⌘` pressed to **copy, clear formatting, and paste** item.*

**기본 동작은 체크박스 2개로 정하고, 그때그때 다른 동작은 수식 키로 즉석에서 낸다.** 설정을 바꾸러 갈 필요가 없다.

> 👉 ★ **[12 §5](12-clipboard-formats.md#5-붙여넣기-모드--네-가지)의 붙여넣기 4모드에 그대로 적용한다.** 우리는 모드를 별도 키(`Shift+Enter` 등)로 뒀는데,
> **"기본값 2개 + 수식 키 조합"** 이 더 적은 설정으로 더 많은 조합을 낸다. **채택.**

### 2-2. ★ "다음 1건만 무시"

`⌥⇧` 클릭 = **바로 다음 복사 한 건만** 기록하지 않는다. 비밀번호를 복사하기 직전에 누르는 동작이다.

> 👉 우리 [FR-C-11](04-feature-scope-and-screens.md#1-1-캡처-fr-c--클립보드에서-우리-쪽으로)(캡처 일시정지)은 **켜고 끄는 토글**뿐이었다.
> **"다음 1건만"은 끄고 다시 켜는 것을 잊지 않게 해 준다** — 실수 방지 설계다. **채택**(FR-C-13).

### 2-3. ★ 개수 상한 + **실사용량 표시**

`Size: 200` 옆에 **`189.3 MB`**. 개수로 관리하되 **그게 디스크에서 얼마인지 즉시 보인다.**

> 👉 [06 §4](06-storage-design.md#4-무제한을-어떻게-다루나)에서 *"개수 무제한보다 용량 상한이 정직하다"* 고 썼는데,
> Maccy는 **개수로 설정하고 용량을 보여주는** 절충을 쓴다. **둘 다 한다** — 개수·기간·용량 상한(FR-H-2) + **실사용량 표시**.

### 2-4. ★ 무시(Ignore)는 **3중 구조**여야 한다

앱 / 클립보드 타입 / 정규식. 그리고 **Maccy가 스스로 "앱 기반은 미덥지 못하다"고 적어 뒀다.**

> 👉 ★ **우리 [FR-S-1](04-feature-scope-and-screens.md#1-6-보안프라이버시-fr-s--차별점)(민감 표식 존중)을 FR-S-2(제외 앱)보다 위에 둔 판단이 실사용 제품의 안내문으로 확인됐다.**
> Ignore 화면은 **Maccy의 3탭 구조를 그대로** 가져온다(+ 우리는 **허용 목록 반전**도 함께).

### 2-5. ★ 종료 시 정리 + **시스템 클립보드까지**

`Clear history on quit` / `Clear the system clipboard too`.

> 👉 우리에게 없던 항목이다. **채택**(FR-S-11) — 특히 **공용 PC·화면 공유 상황**에서 의미가 크다.

### 2-6. ★ 검색 모드를 **설정으로** 둔다

`Search: Exact` 택일(Maccy는 Exact / Fuzzy / Regular expression / Mixed).

> ⚠️ **우리와 판단이 갈리는 지점**이다. [04 §2-2-2](04-feature-scope-and-screens.md#2-2-2--툴바--검색바)에서 *"정규식은 메인창에만, 퀵 팝업은 3초를 지켜야 하니 단순 타입어헤드만"* 이라고 정했다.
> **절충**: **검색 모드를 설정으로 두되**(Maccy 방식) **기본은 `Exact`**, 그리고 **메인창 검색바에는 `.*` 토글**을 함께 둔다 — 설정을 안 건드려도 그 자리에서 바꿀 수 있다.

---

## 3. ★ `registry()` 명세 — nexa-clip

> beep `SettingKind` 어휘를 그대로 쓴다. **출처** 열: `M`=Maccy에서 · `N`=우리 신규 · `B`=beep 설정에서 계승.

### 3-1. 일반 (General)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `app.autostart` | Checkbox | on | 로그인 시 자동 시작 | M |
| `app.update_check` | Checkbox | on | 자동 업데이트 확인 (+ `지금 확인` 버튼) | M |
| `app.lang` | Combo | 시스템 | 언어(ko/en/ja) | B |
| `app.data_path` | 정보행 | — | ★ 데이터 위치(포터블/폴백 **표시만**) | B |

### 3-2. 단축키 (Shortcuts)

| key | 종류 | 기본값(Win/Linux · mac) | 출처 |
|---|---|---|:--:|
| `key.open` | Hotkey | `Ctrl+Shift+V` · `⇧⌘C` | M |
| `key.main_window` | Hotkey | — | N |
| `key.pin` | Hotkey | `Ctrl+P` · `⌥P` | M |
| `key.delete` | Hotkey | `Delete` · `⌥⌫` | M |
| `key.preview` | Hotkey | `Ctrl+Space` · `^Space` | M |
| `key.paste_plain` | Hotkey | `Shift+Enter` | M |
| `key.view_mode_1/2/3` | Hotkey | `Ctrl+1/2/3` | N |

> ★ 각 필드에 **✕(해제) 버튼**과 **충돌 감지 표시**(FR-U-9).

### 3-3. 캡처 (Capture)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `cap.enabled` | Switch | on | 클립보드 감시 | M |
| `cap.text` | Checkbox | on | 텍스트 저장 | M |
| `cap.image` | Checkbox | on | 이미지 저장 | M |
| `cap.files` | Checkbox | on | **파일·폴더 저장** | M |
| `cap.rich` | Checkbox | on | ★ **서식(HTML/RTF) 저장** | N |
| `cap.native_formats` | Checkbox | on | ★ **원본 포맷 보존**(Word·PPT·Excel) → [12](12-clipboard-formats.md) | N |
| `cap.max_rep_mb` | RadioInput | 8 | 표현 1개 상한(MB) | N |
| `cap.max_item_mb` | RadioInput | 32 | 항목 전체 상한(MB) | N |
| `cap.dedup` | Checkbox | on | 같은 내용 재복사 시 최신으로 승격 | B |
| `cap.poll_ms` | RadioInput | 적응형 | (macOS 전용) 폴링 주기 | N |
| `cap.thumb_px` | RadioInput | 96 | ★ **썸네일 긴 변(px)** — 목록 미리보기 품질 ↔ 디스크 → [§3-12](#3-12-시간모션-motion--사용자-요청-08-26) | N |
| `cap.preview_lines` | RadioInput | 5 | 미리보기 최대 줄 수(일반 보기) | N |

### 3-4. 보관 (Storage)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `store.max_items` | RadioInput | 1000 | 최대 항목 수 (+ ★ **실사용량 표시**) | M |
| `store.max_days` | RadioInput | 0(무제한) | 최대 보관 기간 | N |
| `store.max_mb` | RadioInput | 2048 | 최대 용량 | N |
| `store.sort` | Combo | 최근 복사순 | ★ **정렬**: 최근 복사순 / 최초 복사순 / **복사 횟수순** | M |
| `store.pin_to` | Combo | 위 | 고정 항목 위치(위/아래) | M |
| `store.rep_ttl` | Checkbox | on | ★ **표현별 차등 수명**(무거운 원본 형식 먼저 회수) → [12 §7](12-clipboard-formats.md#7-용량--office-클립보드는-크다) | N |
| `store.rep_ttl_days` | RadioInput | 7 | ★ 무거운 원본 형식 회수까지 일수(`store.rep_ttl` on일 때) | N |
| `store.thumb_cache` | RadioInput | 200 | ★ **썸네일 메모리 캐시 장수**(LRU) — 유휴 RSS 직결 → **D-70** | N |

### 3-5. 보안·개인정보 (Privacy) — ★ 우리 차별점

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `sec.respect_marks` | Checkbox | ★ **on** | **비밀번호 관리자 표식 존중**(권장) | M |
| `sec.encrypt_at_rest` | Checkbox | ★ **on** | ★ **기록 암호화**(D-9) | N |
| `sec.ignore_apps` | 목록 편집 | 빈 목록 | 제외 앱 (+ ★ **"목록 외 전부 무시"** 반전) | M |
| `sec.ignore_types` | 목록 편집 | 기본 민감 타입 | ★ **제외 클립보드 타입**(Maccy 권장 방식) | M |
| `sec.ignore_regex` | 목록 편집 | 빈 목록 | 제외 정규식(카드번호·주민번호 등) | M |
| `sec.clear_on_quit` | Checkbox | off | ★ **종료 시 기록 비우기** | M |
| `sec.clear_system_too` | Checkbox | off | ★ **시스템 클립보드까지 비우기** | M |
| `sec.lock` | Checkbox | off | 앱 잠금(암호/생체) — P2 | N |
| `sec.sensitive_ttl` | RadioInput | 0 | 민감 타입 자동 만료(분) | N |

### 3-6. 붙여넣기 (Paste)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `paste.auto` | Checkbox | on | **자동 붙여넣기**(선택 시 원래 창에 입력) | M |
| `paste.plain_default` | Checkbox | off | ★ **항상 평문으로** | M |
| `paste.permission` | 정보행 + 버튼 | — | (mac) **손쉬운 사용 권한 상태 + 요청** | N |
| `paste.app_rules` | 목록 편집 | 빈 목록 | 앱별 규칙(터미널엔 평문 등) — P2 | N |
| `paste.inline_max_mb` | RadioInput | (실측 후) | ★ **즉시 게시 임계값** — 넘는 표현만 지연 렌더링으로 광고 → **D-72** | N |
| `paste.restore_focus_ms` | RadioInput | 120 | ★ 포커스 복원 후 키 주입까지 대기(ms) — 느린 앱 대응 | N |
| (안내) | 정적 텍스트 | — | ★ **수식 키 안내**([§2-1](#2-1--수식-키가-설정을-덮어쓴다)) | M |

### 3-7. 모양 (Appearance)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `ui.theme` | Combo | 시스템 | 테마(시스템/라이트/다크) | B |
| `ui.popup_at` | Combo | 커서 | **팝업 위치**(커서/화면 중앙/마지막 위치) | M |
| `ui.view_mode` | Combo | 간략 | ★ **목록 보기**(일반/간략/한 줄) | N |
| `ui.density` | Combo | 보통 | 밀도 | B |
| `ui.image_height` | RadioInput | 40 | 이미지 행 높이(px) | M |
| `ui.preview_auto` | Checkbox | on | **미리보기 자동 열기** | M |
| `ui.preview_delay` | RadioInput | 1500 | 미리보기 지연(ms) | M |
| `ui.highlight` | Combo | 굵게 | ★ **검색 일치 강조**(굵게/기울임/밑줄/없음) | M |
| `ui.show_app_icons` | Checkbox | on | 출처 앱 아이콘 표시 | M |
| `ui.show_color_swatch` | Checkbox | on | 색상 견본 표시 | M |
| `ui.show_special_symbols` | Checkbox | on | 특수 기호 표시 | M |
| `ui.show_search` | Combo | 항상 | 검색창 표시(항상/입력 시) | M |
| `ui.show_footer` | Checkbox | on | 하단 바 표시 | M |
| `ui.tray_recent_n` | RadioInput | 8 | ★ **트레이 메뉴 최근 항목 수(5~10)** | N |
| `ui.tray_show_recent` | Checkbox | on | ★ 트레이 아이콘 옆 최근 항목 표시 | M |
| `ui.dock_icon` | Checkbox | on | ★ **Dock 아이콘 표시**(mac 전용 — 끔 = Accessory·메뉴바에서만 · 09-04) | N |
| `ui.font.*` | 폰트 슬롯 | — | 글꼴·크기(beep 슬롯 구조 그대로) | B |
| `hist.clear` | Action | — | ★ **기록 모두 삭제**(09-04 · 고급) — **2단계 확인**: 첫 클릭 = 경고 노트 + 2초 무장 · 그 안의 둘째 클릭 = 고정 제외 전부 삭제(메모리 + 암호화 저장소) · 2초 지나면 자동 해제 · 값 키 없음(행위) | N |
| `ui.font_mono` | FontFace | (비움) | ★ **고정폭 글꼴**(09-04) — 터미널·코드 리치 런의 Mono 슬롯 · 비우면 Nerd Font → D2Coding → JetBrains Mono → Cascadia → Consolas → Menlo → DejaVu 순 첫 설치본 · 주 글꼴에도 폴백으로 붙어 PUA 아이콘 글리프가 산다 · 재시작 후 적용 | N |

### 3-8. 검색 (Search)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `find.mode` | Combo | **정확히** | ★ **검색 방식**(정확히/유사/정규식/혼합) — [§2-6](#2-6--검색-모드를-설정으로-둔다) | M |
| `find.case` | Checkbox | off | 대소문자 구분 | M |
| `find.hangul_compose` | Checkbox | on | ★ **한글 조합 중 검색**(FR-F-2) | N |

### 3-9. 동기화 (Sync) — M2 · ★ 09-03~04 실구현 반영

| key | 종류 | 기본값 | 설명 |
|---|---|---|---|
| `sync.enabled` | Switch | ★ **off** | 끄면 동기화 자체 끔(릴레이·LAN·세션) · 꺼져 있으면 아래 전부 잠금 |
| `sync.device_name` | Text | — | 기기 표시 이름(비면 호스트명 정제 → `clip-{지문4}` 폴백 · 프로필 실행은 `-{프로필}` 접미) |
| `sync.handle` | Text | — | 핸들([09 §6](09-identity-and-pairing.md)) · 입력 시 패스프레이즈 자동 추천 |
| `sync.passphrase` | Text(비밀) | — | 페어링 패스프레이즈 · 눈(보기) · 생성(2단 확인 · 생성 시 표시) · 서버로 안 감 |
| `sync.relay` | RadioInput | `beepd.sosomlab.com` | 공식 릴레이 · 직접 입력 · **`none`** = 같은 네트워크만(포트·Test·Disconnect 잠금 · Test 없이 즉시 적용) |
| `sync.port` | RadioInput | `47300` | 릴레이 TCP 제어 포트 |
| `sync.retry` | Radio | `normal` | 재시도 정책 — 실패 n회째 = base×2^(n−1)(상한 · ±20% 지터 · 성공 시 초기화) · normal 5s→5분 · patient 15s→15분 · eager 2s→1분 |
| `sync.test` | Action | — | 릴레이 접속 시험 → 성공 = `sync.enabled` 자동 켬 + 러너 (재)기동 · 연결 중엔 잠금 · 실행 시 자동 Test 노트 |
| `sync.disconnect` | Action | — | 릴레이 세션 해제(연결 중에만 활성 · Connected 자리에 Disconnected) |
| `sync.devices` | ★ DeviceList | — | 만난 기기 행별 **[승인/해제][삭제]** + 온라인 행 6자리 대조 코드 · `연결됨 (LAN/relay)` · 승인 전엔 전파 없음 |
| `ui.dedup_view` | (숨김 · 메인 툴바 토글) | **on** | 중복 제외 보기(같은 내용 한 행 · 로컬 우선 · 출처 메타) — 팝업도 동일 |

**설계만 있고 미구현**(D-20·DR-29): `sync.overwrite_mode` · `sync.remote_media` · `sync.pause` · `sync.relay_received` · 신뢰 목록 자동 승인 토글 → T-27 잔여·T-24.

### 3-10. 고급 (Advanced)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `adv.ignore_next` | 동작 버튼 | — | ★ **다음 복사 1건만 무시**([§2-2](#2-2--다음-1건만-무시)) | M |
| `adv.portable` | 정보행 | — | 포터블 모드 여부 표시 | B |
| `adv.log` | Checkbox | off | 진단 로그(로컬만) | B |
| `adv.cli` | Checkbox | off | CLI 제어 활성 — P2 | N |
| `adv.reset` | 위험 버튼 | — | 초기화(확인 필요) | B |

### 3-11. 정보 (About)

버전 · 라이선스(PolyForm NC) · 저장소 · 데이터 경로 · 오픈소스 고지.

### 3-12. 시간·모션 (Motion) — ★ 사용자 요청 08-26

> *"개수나 시간 등의 제약이나 설정은 설정 화면에서 조정 가능하도록. **1000ms 글로우 처리 시간 등도 포함**."*
>
> [DR-34](10-decision-record.md)가 hover 1000ms를 확정했지만, 그건 **기본값**이지 못 박은 상수가 아니다.
> 이 절이 [`nclip_ctl::tokens`](25-design-system.md)의 모션 상수를 **런타임 설정으로 승격**한다.

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `ui.motion` | Combo | ★ **시스템 따름** | ★ **모션**: 시스템 따름 / 표준 / 빠름 / **끔** | N |
| `ui.hover_in_ms` | RadioInput | **1000** | ★ **hover 페이드인**([DR-34](10-decision-record.md)) | N |
| `ui.hover_out_ms` | RadioInput | 220 | hover 페이드아웃 | N |
| `ui.popup_ms` | RadioInput | 90 | 팝업 등장(⚠️ 상한 **120ms** — [§3-12-2](#3-12-2--상한은-설정이-아니라-코드에-남긴다)) | N |
| `ui.state_ms` | RadioInput | 90 | 상태 레이어 전이(선택·포커스) | N |
| `ui.typeahead_reset_ms` | RadioInput | 800 | 타입어헤드 입력 초기화 대기 | B |
| `ui.toast_ms` | RadioInput | 3000 | 알림(토스트) 표시 시간 | N |
| (미리보기 지연) | → [§3-7](#3-7-모양-appearance) `ui.preview_delay` | 1500 | 이미 있음 | M |

#### 3-12-1. ★ `ui.motion`이 먼저다 — 개별 수치는 그 아래

**개별 ms를 먼저 보게 하면 [DR-26](10-decision-record.md)(저빈도 기능 배제)을 정면으로 어긴다.**
99%의 사용자는 숫자 7개가 아니라 **"덜 움직이게 해 줘"** 를 원한다.

| `ui.motion` | 동작 |
|---|---|
| **시스템 따름**(기본) | OS의 *동작 줄이기*(Windows `SPI_GETCLIENTAREAANIMATION` · macOS `reduce motion` · GNOME `enable-animations`)를 읽어 **표준 ↔ 끔** 을 자동 선택 |
| **표준** | 아래 개별 수치 그대로 |
| **빠름** | 개별 수치 × 0.4 |
| **끔** | 전부 0 — ★ **페이드가 아니라 즉시 전환**(잔상 0) |

> 개별 ms 7개는 **고급(Advanced) 접힘 아래**에 둔다. `ui.motion`이 `표준`이 아닐 때는 **비활성 + 계산된 실효값 표시**.
> ★ 이러면 설정이 늘어도 **첫 화면의 인지 부담은 그대로**다.

#### 3-12-2. ⚠️ 상한은 설정이 아니라 코드에 남긴다

[25 §3-7](25-design-system.md)이 **팝업 120ms 상한을 컴파일 타임 assert**로 못 박았다.
런타임 설정이 생겨도 **그 assert를 지우지 않는다** — 대신 이렇게 나눈다.

| 층 | 역할 |
|---|---|
| **컴파일 타임 assert** | **기본값**이 상한 안에 있는지 — 개발자가 상수를 잘못 고치는 것을 막는다 |
| **런타임 clamp** | 설정에서 읽은 값을 `[0, 상한]`으로 자른다 — 설정 파일을 손으로 고친 경우를 막는다 |

★ **둘은 대체 관계가 아니다.** assert는 *우리*를, clamp는 *파일*을 막는다.

#### 3-12-3. ⚠️ 이 승격이 코드에 요구하는 것

`tokens`의 모션 값이 **`const`에서 런타임 값으로 바뀐다.** 파급을 미리 적어 둔다.

| # | 파급 |
|:--:|---|
| **1** | 모션 상수는 `DrawCtx`(또는 그 아래 `Motion` 구조체)를 타고 내려간다 — 색·간격 토큰은 `const`로 남는다 |
| **2** | ★ **진행 중인 페이드에 새 값이 적용되는 순간**을 정해야 한다 → **다음 페이드부터**(진행 중인 것은 원래 값으로 끝낸다). 안 그러면 설정 창에서 숫자를 돌릴 때 화면이 튄다 |
| **3** | `끔`(0ms)은 `Fade`가 **0으로 나누지 않도록** 별도 분기 — 페이드 객체를 만들지 않고 목표 상태를 즉시 쓴다 |

---

## 4. Maccy에 있는데 **우리가 안 할 것**

| 항목 | 이유 |
|---|---|
| `Show menu icon` 아이콘 **선택** | 트레이 아이콘 교체는 부가 가치가 낮다(P2로 보류) |
| `Show title before search field` | 우리 팝업은 제목 줄이 없다 |
| `defaults write` 기반 제어 | macOS 전용 관용구 — 우리는 **CLI**로 3-OS 공통 제공(P2) |

## 5. 우리만 있는 것 (Maccy 대비 상향)

| 항목 | 근거 |
|---|---|
| ★ **기록 암호화**(기본 켜짐) | [03 §6-2 ②](03-competitive-landscape.md#6-2-nexa-clip의-자리--세-개의-차별점) |
| ★ **원본 포맷 보존**(Word·PPT·Excel) | [12 §3](12-clipboard-formats.md#3--핵심--htmlrtf만으로는-부족하다) |
| ★ **보기 모드 3종** | [04 §2-2-3](04-feature-scope-and-screens.md#2-2-3--④-목록--세-가지-보기-모드) |
| ★ **기기 간 동기화**(P2P·E2E) | [05](05-multi-device-sharing.md)·[08](08-clipboard-propagation.md) |
| ★ **3-OS 동일 화면** | DR-1 |
| 보관 기간·용량 상한 | Maccy는 개수만 |
| 그룹·보드·스니펫 | M3 |

---

## 6. 열린 결정

| # | 결정할 것 | 선택지 | 권장 |
|---|---|---|---|
| **D-38** | 수식 키 동작 모델 | ★ **Maccy 방식**(기본값 + `⌥`/`⌘`/`⇧⌘` 즉석 변형) vs 모드별 고정 키 | ✅ **닫힘 → DR-40**(08-31): Maccy 방식 |
| **D-39** | 검색 모드 배치 | 설정 택일만 · 검색바 토글만 · ★ **둘 다** | ✅ **닫힘 → DR-40**(08-31): 둘 다 |
| **D-40** | 기본 보관 개수 | Maccy 200 · ★ **1000** · [06](06-storage-design.md) 권장 10000 | ✅ **닫힘 → DR-40**(08-31): **1000** |
| **D-41** | `sec.clear_on_quit` 기본값 | off(Maccy) vs on | ★ off — 켜면 제품의 존재 이유가 사라진다 |
| **D-70** | 썸네일 정책 — 크기(`cap.thumb_px`) · 저장 포맷 · 캐시 장수(`store.thumb_cache`) | 96px raw RGBA · 128px · 256px | 🔴 **인코더가 없다** — [§6-1](#6-1--d-70-썸네일이-예산을-정한다) |
| ~~D-71~~ | ~~민감 항목을 수렴 암호화에서 제외~~ | — | ⛔ **불필요해짐** — `blob_id = HMAC(K_user, H(평문))`(키 있는 수렴 암호화)로 가면 확인 공격 오라클이 **애초에 없다**(08-26 정정) |
| **D-74** | `sec.encrypt_at_rest = off`의 의미 | (a) 완전 평문 · ★ **(b) 기기 로컬 키로는 항상 암호화, 패스프레이즈만 안 받음** | ✅ **닫힘 → DR-38**(08-31): **(b)** · 기본 = 켜짐 |
| **D-72** | 지연 렌더링 임계값(`paste.inline_max_mb`) · `WM_RENDERALLFORMATS` 처리 | 실측 후 | 🔴 [§6-2](#6-2--d-72-지연-렌더링은-키-주입과-부딪힌다) |
| **D-73** | `ui.motion` 개별 수치 노출 범위 | 고급 접힘 · 별도 탭 · 숨김(설정 파일만) | ★ 고급 접힘([§3-12-1](#3-12-1--uimotion이-먼저다--개별-수치는-그-아래)) |

### 6-1. ★ D-70 — 썸네일이 예산을 정한다

`store.max_items = 1000`(확정)에서 인덱스는 ~0.3MB로 문제가 아니다. **문제는 썸네일이다.**

| 긴 변 | 장당(raw RGBA) | 1,000장 디스크 | 판정 |
|---:|---:|---:|---|
| 64px | 16KB | 16MB | 목록에서 너무 작다 |
| **96px** | **36KB** | **36MB** | ★ 권장 — `ui.image_height` 기본 40px의 2배 여유 |
| 128px | 64KB | 64MB | 고DPI에서 선명 |
| 256px | 256KB | **256MB** | ⚠️ 과하다 |

⚠️ **압축 저장을 하려면 PNG *인코더*가 필요한데 우리에겐 디코더밖에 없다**([27 §4-2](27-capture-cases.md)).
[DR-8](10-decision-record.md)(외부 crate 0)을 지키려면 **raw RGBA 무압축**이고, 그러면 위 표가 그대로 디스크 비용이다.
→ 96px raw를 권장한다. 메모리는 `store.thumb_cache`(LRU 200장 ≈ 7MB)가 따로 막는다.

### 6-2. ⚠️ D-72 — 지연 렌더링은 키 주입과 부딪힌다

임계값 하이브리드(사용자 확정 08-26)를 쓰면 함정이 둘 생긴다.

| # | 함정 | 처리 |
|:--:|---|---|
| **1** | **우리가 죽으면 클립보드가 빈다** — 지연 광고한 표현은 우리 프로세스가 살아 있어야 렌더된다 | 종료 시 `WM_RENDERALLFORMATS` 처리(mac은 `pasteboard(_:provideDataForType:)`). ★ **처리를 빠뜨리면 앱 종료가 사용자의 클립보드를 지운다** |
| **2** | ★ **자동 붙여넣기와 데드락** — 키를 주입한 직후 대상 앱이 데이터를 요청하는데 우리는 아직 창 복원 중이다. Windows 클립보드는 **전역 잠금**이다 | 자동 붙여넣기 경로에서는 **지연 렌더링을 쓰지 않는다**(임계값 무시하고 즉시 전량 게시). 지연은 **사용자가 직접 `Ctrl+V`** 하는 경로에만 |

---

## ★ 개수 설정은 **숫자를 그대로** 보여 준다 (08-26 · 사용자 확정)

> *"Maximum items가 최대 보관 클립보드 수라면 숫자로 직접 지정하도록 숫자로 표현해줘. 200, 1000 등등"*

`SettingKind::Number { presets, suffix }` 를 새로 뒀다. [`Radio`]와 달리 **라벨을 번역하지 않는다** —
후보 문자열이 곧 화면에 보이는 값이다.

| 키 | 후보 | 기본 | 직접 입력 범위 |
|---|---|:--:|---|
| `store.max_items` | `200` · `500` · `1000` · `5000` · `10000` | **1000** | 10 ~ 100000 |
| `ui.tray_recent_n` | `5` · `8` · `10` · `15` · `20` | **8** | 3 ~ 20 |

★ **왜 바꿨나** — *"보통"* 이 몇 개인지 알려면 설명을 읽어야 하지만 **`1000`은 그 자체로 답**이다.
개수처럼 **단위가 분명한 값**에 추상 라벨을 씌우면 정보가 사라진다.

⚠️ **범위를 두는 이유** — 24시간 상주 앱이다. `0`이면 히스토리가 없는 것과 같고,
`1000000`이면 메모리와 검색 예산([00 §2](00-vision.md) 검색 ≤16ms)이 무너진다.
범위를 벗어나면 경고를 띄우고 **직전 확정값으로 원복**한다(기존 검증 배선 그대로).

> 글꼴 크기(`font.*.size`)는 **바꾸지 않았다** — 거기서는 *작게/보통/크게* 가
> 관례이고, 사용자가 원하는 것도 "몇 px"가 아니라 "지금보다 크게"다.
