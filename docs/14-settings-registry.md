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

### 3-4. 보관 (Storage)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `store.max_items` | RadioInput | 1000 | 최대 항목 수 (+ ★ **실사용량 표시**) | M |
| `store.max_days` | RadioInput | 0(무제한) | 최대 보관 기간 | N |
| `store.max_mb` | RadioInput | 2048 | 최대 용량 | N |
| `store.sort` | Combo | 최근 복사순 | ★ **정렬**: 최근 복사순 / 최초 복사순 / **복사 횟수순** | M |
| `store.pin_to` | Combo | 위 | 고정 항목 위치(위/아래) | M |
| `store.rep_ttl` | Checkbox | on | ★ **표현별 차등 수명**(무거운 원본 형식 먼저 회수) → [12 §7](12-clipboard-formats.md#7-용량--office-클립보드는-크다) | N |

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
| `ui.font.*` | 폰트 슬롯 | — | 글꼴·크기(beep 슬롯 구조 그대로) | B |

### 3-8. 검색 (Search)

| key | 종류 | 기본값 | 라벨 | 출처 |
|---|---|---|---|:--:|
| `find.mode` | Combo | **정확히** | ★ **검색 방식**(정확히/유사/정규식/혼합) — [§2-6](#2-6--검색-모드를-설정으로-둔다) | M |
| `find.case` | Checkbox | off | 대소문자 구분 | M |
| `find.hangul_compose` | Checkbox | on | ★ **한글 조합 중 검색**(FR-F-2) | N |

### 3-9. 동기화 (Sync) — M2

| key | 종류 | 기본값 | 출처 |
|---|---|---|:--:|
| `sync.enabled` | Switch | ★ **off** | N |
| `sync.handle` | TextBox | — | 핸들([09](09-identity-and-pairing.md)) |
| `sync.passphrase` | TextBox(비밀) | — | 페어링 패스프레이즈 |
| `sync.devices` | 목록 | — | ★ **기기 관리**(추가·폐기·이력) |
| `sync.overwrite_mode` | Combo | (D-20) | 받은 항목 처리(기록만/자동 덮어쓰기/유휴 시) |
| `sync.remote_media` | Combo | 메타 우선 | 원격 이미지 정책 |
| `sync.relay` | TextBox | — | 릴레이 서버 주소(선택) |
| `sync.pause` | Switch | off | ★ 전파 일시 정지(회의·화면 공유) |
| `sync.relay_received` | Checkbox | ★ **off** | ★ **받은 항목을 내 기기들에 재전파**(DR-29 F-2) |
| (신뢰 목록 각 행) | 토글 | off | ★ **이 사람은 자동 승인**(DR-29 T-9 — 사람 단위) |

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
| **D-38** | 수식 키 동작 모델 | ★ **Maccy 방식**(기본값 + `⌥`/`⌘`/`⇧⌘` 즉석 변형) vs 모드별 고정 키 | ★ Maccy 방식 — 설정을 덜 건드리게 한다([§2-1](#2-1--수식-키가-설정을-덮어쓴다)) |
| **D-39** | 검색 모드 배치 | 설정 택일만 · 검색바 토글만 · ★ **둘 다** | ★ 둘 다([§2-6](#2-6--검색-모드를-설정으로-둔다)) |
| **D-40** | 기본 보관 개수 | Maccy 200 · ★ **1000** · [06](06-storage-design.md) 권장 10000 | 🔴 인덱스 메모리와 직결([06 §2-1](06-storage-design.md#2-1-인덱스-레코드--작게-유지하는-것이-전부)) |
| **D-41** | `sec.clear_on_quit` 기본값 | off(Maccy) vs on | ★ off — 켜면 제품의 존재 이유가 사라진다 |
