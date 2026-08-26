# 24 · 참조 제품 설정 화면 심층 연구 — CopyQ · Maccy · Paste

> **근거**: 사용자가 제공한 **CopyQ Preferences 캡처 10장**(2026-08-26) + [14 Maccy 설정 실측](14-settings-registry.md) + [17 UI 해부](17-reference-ui-teardown.md).
> **목적**: 우리 설정 화면([13 §2-3](13-ui-reuse-from-beep.md) 프레임워크 + [`settings_registry`](../crates/nclip-ui/src/settings_registry.rs))의 **방향을 실물로 검증**한다.
>
> ★ 사용자가 **매일 쓰는 두 제품**의 설정을 통째로 본 것이라, 여기 나온 것은 취향이 아니라 **검증된 선택**이다.

---

## 1. CopyQ Preferences — 전수 (캡처 10장)

좌측 카테고리 **9개**: General · Layout · History · Tray · Notifications · Tabs · Items · Shortcuts · Appearance

### 1-1. General

언어 · `Wrap long text` · `항상 위에` · `Close When Unfocused` · ★ **`Open windows on current screen`** ·
`Confirm application exit` · `Vi style navigation` · `Save Filter History` · `Auto-complete Commands` ·
**Clipboard Manipulation** → `Store clipboard`

### 1-2. Layout — ★ **투명도가 여기 있다**

| 그룹 | 항목 |
|---|---|
| Show/Hide | `Hide tabs` · `Hide toolbar` · `Hide toolbar labels`(체크됨) · `Hide main window` |
| **Layout and Transparency** | `Tab Tree` · `Show Item Count` · ★ **`Focused transparency: 0%`** · ★ **`Unfocused transparency: 0%`** |

> ★★ **포커스 여부로 투명도를 나눈다.** 이건 영리하다 — **읽을 때(포커스)는 불투명, 비켜 있을 때는 반투명**.
> [23 §3-2](23-alpha-rendering.md#3-2-판단--지금-하지-않는다)에서 내가 *"목록은 글자를 읽는 화면이라 뒤가 비치면 대비가 무너진다"* 며
> 창 투명(L2)을 접었는데, **이 분리가 그 반론을 무력화한다** → [§4-1](#4-1-우리-판단을-뒤집는-것-3가지).

### 1-3. History — ★ **"활성화 후 동작"이 4개로 분해돼 있다**

`Tab for storing clipboard` · **`Maximum number of items: 200`** · `Unload tab after interval` ·
`External editor command` · `Save edited item with Ctrl+Return` · `Show simple items` ·
`Search for numbers` · `Activate item with single click`

> **After item is activated (double-click or Enter), copy it to clipboard and …**
> ☑ `Move item to the top` ☑ `Close main window` ☑ **`Focus last window`** ☑ **`Paste to current window`**

★ **우리가 "자동 붙여넣기" 한 토글로 뭉쳐 둔 것이 실은 이 넷의 조합**이다([§4-1](#4-1-우리-판단을-뒤집는-것-3가지)).

### 1-4. Tray

`Disable tray` · `Show commands for clipboard content` · **`Number of items in tray menu: 5`** ·
`Show current tab in menu` · ★ **`Paste activated item to current window`** ·
★ **`Show image preview as menu item icon`**

> ★ **트레이 메뉴 항목에 이미지 미리보기를 아이콘으로 넣는다.** [04 TR-2](04-feature-scope-and-screens.md)에서
> 내가 *"네이티브 메뉴에 비트맵은 OS별 제약"* 이라며 타입 아이콘만 쓰기로 했는데, **CopyQ는 실제로 한다.**

### 1-5. Notifications

`Use native notifications` · `Notification position: Bottom Right` · `Interval in seconds` ·
`Number of lines for clipboard notification` · **Notification Geometry**(가로/세로 오프셋 · 최대 폭/높이)

### 1-6. Tabs

탭별 `Maximum number of items: default` · `항목 저장`

### 1-7. Items — ★ **항목 타입이 플러그인이고 순서가 있다**

```
[↑][↓][⇈][⇊]        ← 순서 조정
☑ Images       → Maximum Image Width: 320 / Height: 240 / Image editor / SVG editor
☑ 암호화
☑ FakeVim
☑ 노트
☑ Pinned Items
☑ Synchronize
☑ Tags
☑ Text
```

★ **순서가 렌더 우선순위**다 — 위에 있는 플러그인이 먼저 항목을 그린다.
우리 **표현(Representation)** 개념과 정확히 대응한다([12](12-clipboard-formats.md)).

### 1-8. Shortcuts — ★ **행위 목록에 키를 붙인다**

`Global` / `Application` 두 탭. 전역 목록(발췌):

| 행위 | 바인딩 |
|---|---|
| ★ **마우스 커서 아래 메인 윈도우 보이기** | `Alt+Shift+C` |
| ★ **클립보드를 일반 문자로 붙여넣기** | `Alt+Shift+X` |
| 메인 윈도우 보이기/숨기기 · 트레이 메뉴 보이기 | — |
| 클립보드 수정 · 첫번째 항목 수정 · 두번째 항목 복사 | — |
| 액션 대화창 · 새 항목 만들기 | — |
| 다음/이전 항목 복사 | — |
| 클립보드 저장 사용안함 / 사용함 | — |
| ★ **붙여넣고 다음 복사** / **붙여넣고 이전 복사** | — |
| 스크린샷 찍기 · 현재 날짜 및 시간 붙여넣기 | — |

★ **사용자가 필수로 확정한 두 기능이 CopyQ에 그대로 있고 키까지 붙어 있다**([DR-24](10-decision-record.md)) — 검증됐다.
★ 각 행의 `+` 버튼 = **한 행위에 키를 여러 개** 바인딩할 수 있다.
★ **`붙여넣고 다음 복사`** 가 스택 붙여넣기(FR-P-5)의 실체다.

### 1-9. Appearance — ★ **실시간 미리보기 + 테마 파일**

색 테이블 **8행 × 3열**(보통·선택됨·찾음·편집기·Alternate·Tooltips·번호·Notification × 배경·Foreground·글꼴)
· `Show Number` · `Scrollbars` · `System Icons` · `Antialias` · `Set colors for tabs, toolbar and menus`
· **테마 불러오기 / 저장 / 초기화 / 수정** · ★ **우측에 실시간 미리보기 목록**

---

## 2. 세 제품 비교

| 축 | **Maccy** | **CopyQ** | **Paste**(참고) |
|---|---|---|---|
| 카테고리 수 | 6 | **9** | 적음 |
| 한 페이지 밀도 | 낮음(여백 큼) | ★ **매우 높음** | 낮음 |
| **설정 검색** | ✕ | ✕(Shortcuts에만 `찾기`) | ✕ |
| 실시간 미리보기 | ✕ | ★ **있음**(Appearance) | ✕ |
| 테마 파일 | ✕ | ★ **불러오기/저장** | ✕ |
| 색 커스터마이즈 | ✕ | ★ **24개 직접 지정** | ✕ |
| 단축키 화면 | 4개 필드 | ★ **행위 15+ 목록** | 소수 |
| 학습 곡선 | ★ 거의 0 | ⚠️ **높다** | 낮음 |

> ★ **CopyQ의 "학습 곡선이 악명 높다"는 평가의 실체가 이 설정 화면에 다 있다** —
> 9개 카테고리 × 빽빽한 페이지 × 검색 없음. **기능이 많아서가 아니라 찾을 수 없어서** 어렵다.

---

## 3. ★ 우리 설정 화면의 자리

| | Maccy | CopyQ | ★ **Nexa Clip** |
|---|:--:|:--:|:--:|
| 깊이 | 얕다 | 깊다 | ★ **깊다** |
| 찾기 | — | ✕ | ★★ **설정 검색**(레지스트리 단일 원천) |
| 구조 | 상단 탭 | 좌측 목록 | ★ **좌측 카테고리 + 검색**(VS Code 방식) |

> ★★ **이 조합이 두 제품 모두에 없다.** CopyQ만큼 담으면서 **찾을 수 있게** 하는 것이 우리 차별점이고,
> 그건 이미 [`settings.rs`](../crates/nclip-ui/src/settings.rs) 프레임워크가 **구조적으로 보장**한다 —
> 렌더와 검색이 같은 원천을 읽으므로 *"화면에 있는데 검색 안 되는 설정"* 이 불가능하다.

---

## 4. 이 연구로 바뀌는 것

### 4-1. 우리 판단을 뒤집는 것 3가지

| # | 내 초판 판단 | CopyQ 실물 | 결론 |
|:--:|---|---|---|
| **①** | **창 투명(L2) 안 함** — *"글자를 읽는 화면이라 대비가 무너진다"*([23 §3-2](23-alpha-rendering.md)) | ★ **포커스/비포커스 투명도를 나눠서** 제공 | ★ **재검토** — 포커스 시 불투명이면 가독성 문제가 없다 → [D-55](#5-열린-결정) · [DR-25 정정 검토](10-decision-record.md) |
| **②** | 트레이 메뉴는 **타입 아이콘만**(TR-2) — *"네이티브 메뉴 비트맵은 제약"* | ★ **이미지 미리보기를 메뉴 아이콘으로** | ★ **재검토** — 실제로 되는 것이 확인됐다 → [D-56](#5-열린-결정) |
| **③** | **자동 붙여넣기 = 토글 하나** | ★ **4개로 분해**(맨 위로 이동 / 창 닫기 / **직전 창 포커스** / **현재 창에 붙여넣기**) | ★ **절충** — 기본은 프리셋 한 줄, **고급에서 분해** → [§4-2](#4-2-우리가-택하는-절충) |

### 4-2. 우리가 택하는 절충 — "기본은 Maccy, 깊이는 CopyQ"

| 항목 | 기본 화면 | 고급 |
|---|---|---|
| **활성화 후 동작** | `자동 붙여넣기` **토글 하나** | ▸ 펼치면 4개 분해(맨 위로·창 닫기·포커스 복원·붙여넣기) |
| **색** | **테마 3택**(시스템/다크/라이트) + **액센트 1색** | ▸ 테마 파일 불러오기/저장(CopyQ 선례) |
| **단축키** | 자주 쓰는 **6개** | ▸ 전체 행위 목록 + 다중 바인딩 |
| **항목 타입** | 타입별 on/off | ▸ **표현 렌더 우선순위**(CopyQ Items 순서 개념) |

> ★ **원칙**: **처음 여는 사람이 보는 것과 깊이 파는 사람이 보는 것을 나눈다.**
> CopyQ는 둘을 한 화면에 쏟아서 어려워졌고, Maccy는 깊이를 아예 없애서 한계가 생겼다.

### 4-3. 그대로 채택할 것

| 항목 | 출처 | 왜 |
|---|---|---|
| ★ **실시간 미리보기**(모양 카테고리) | CopyQ | 색·밀도·보기 모드는 **글로 설명할 수 없다** — 보여 주는 게 맞다 |
| ★ **테마 파일 불러오기/저장** | CopyQ | 사용자 간 공유·백업. 우리는 **JSON 한 장**으로 |
| **알림 위치·오프셋** | CopyQ | 전파 토스트([08](08-clipboard-propagation.md))에 그대로 필요 |
| **`Open windows on current screen`** | CopyQ | 멀티 모니터에서 팝업이 엉뚱한 화면에 뜨는 문제 — [FR-U-3](04-feature-scope-and-screens.md)에 포함 |
| **`Close When Unfocused`** | CopyQ | 팝업은 포커스를 잃으면 닫히는 게 자연스럽다 |
| **`붙여넣고 다음 복사`** | CopyQ | 스택 붙여넣기(FR-P-5)의 구체 형태 |

### 4-4. 하지 않을 것

| 항목 | 왜 |
|---|---|
| **24개 색 직접 지정**(Appearance 8×3) | ★ 과도하다. 사용자가 **읽을 수 없는 조합**을 만들 수 있다 → 테마 프리셋 + 액센트 1색 |
| `Vi style navigation` | 우리 대상 사용자가 아니다 |
| `FakeVim`·`외부 편집기 명령` | 내장 편집(S4)으로 충분 |
| 탭별 개별 상한(Tabs) | 그룹은 **분류**지 별도 저장소가 아니다 |

---

## 5. 열린 결정

| # | 결정할 것 | 선택지 | 권장 |
|---|---|---|---|
| **D-55** | 창 투명(L2) | 하지 않음(현 DR-25) · ★ **포커스/비포커스 분리 투명도** · 단일 값 | 🔴 **재검토 — CopyQ 방식이 가독성 반론을 해소**한다 |
| **D-56** | 트레이 메뉴 이미지 미리보기 | 타입 아이콘만(현 TR-2) · ★ **작은 미리보기 아이콘** | 🔴 실기 확인 후(OS별 제약 실측) |
| **D-57** | 활성화 후 동작 | 토글 하나 · ★ **기본 토글 + 고급 4분해** | ★ 절충안([§4-2](#4-2-우리가-택하는-절충)) |
| **D-58** | 테마 커스터마이즈 범위 | 프리셋만 · ★ **프리셋 + 액센트 1색 + 테마 파일** | ★ 중간 |
