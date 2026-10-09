//! 설정 화면 — **VS Code 방식** · 커스텀 컨트롤 툴킷으로 구성.
//!
//! ★ **출처**: `nexa-beep` `crates/nbeep-ui/src/settings.rs` @ `7118252` — **프레임워크를 이식**하고
//! **`registry()`만 우리 것으로 교체**했다([docs/13 §2-3](../../../docs/13-ui-reuse-from-beep.md)).
//! 화면 코드는 손대지 않았다 — 레지스트리가 단일 원천이라 **데이터만 갈아끼우면 되는 구조**였다.
//! 우리 항목 정의는 [`crate::settings_registry`]([docs/14](../../../docs/14-settings-registry.md)).
//!
//! 핵심 발명은 **Entry 레지스트리 단일 원천**이다: 영속 설정 전부가 [`registry`]에 등록되고,
//! 렌더와 검색이 같은 원천을 읽는다 — "화면에 있는데 검색 안 되는 설정"이 구조적으로 불가능하다.
//!
//! ## 컨트롤 구성(사용자 확정 08-09 — 자체 렌더 전면 교체)
//!
//! | 요소 | 컨트롤 |
//! |---|---|
//! | 검색 | [`TextBox`](placeholder·Beam 캐럿) |
//! | 카테고리 사이드바 | [`TreeView`](검색 중 매치 카테고리 + "(N)") |
//! | 택일 설정 | [`Combo`](드롭다운 · 선택 ✓) |
//! | on/off 설정 | [`Checkbox`](nexa_ctl::controls::Checkbox) |
//! | 글꼴 영역 | [`TextBox`] 글꼴명 + [`Combo`] 크기 |
//!
//! 값 반영은 기존 계약 그대로 — **즉시 적용**([`SettingsWidget::take_changes`] 폴링), 영속은
//! M2-5(Repository 포트). i18n: 라벨은 [`Msg`] 키, 검색은 **전 언어 매치**.

use nclip_core::{current_lang, tr, Lang, Msg};
use nexa_ctl::controls::{
    Button, ColorPicker, Combo, ComboControl, ComboItem, Control, LabelSide, ListEditor,
    PositionPicker, ScrollBars, Switch, TextBox, TreeControl, TreeModel, TreeNode, TreeView,
};
use nexa_ctl::draw::{DrawCtx, FontSlot};
use nexa_ctl::event::{InputEvent, Key};
use nexa_ctl::geom::{Point, Rect};
use nexa_ctl::theme::Theme;
use nexa_ctl::tokens::Fade;
use nexa_ctl::widget::{Invalidations, Widget};
use std::collections::HashMap;

// 크기 콤보 후보(beep 레지스트리 전용) — 우리 레지스트리가 해당 항목을 갖게 되면 되살린다.

/// 크기 기본값 — 순서와 무관하게 '보통' 고정.
/// 폰트 크기 프리셋(08-18 2차 확정 — **값은 절대 px · 라벨은 이름만**):
/// 크기 안내는 Base UI 설명문이 진다("Normal (16px)" 병기는 원복).
/// 기본은 **전 슬롯 Normal(16px)** · Small = 14px(사용자 확정).
const FONT_SIZE_OPTS: &[(&str, Msg)] = &[
    ("14", Msg::SizeSmall),
    ("16", Msg::SizeNormal),
    ("18", Msg::SizeLarge),
    ("22", Msg::SizeXLarge),
];

/// 폰트 크기 기본(px) — 전 슬롯 공통 Normal(사용자 확정 08-18).
const FONT_SIZE_DEFAULT: &str = "16";

// (동일 — SIZE_OPTS 부활 시 함께)

/// 설정 **화면에는 없지만** 영속되는 키(M3-17 프로필 화면이 쓴다) — 기본 빈 문자열.
/// ⚠ 이메일·전화는 PII다 — 평문 settings.cfg 보관은 잠정이며 M2-5b(암호화 저장)로
/// 이관 후보(journal 08-11 명기).
const HIDDEN_KEYS: &[&str] = &[
    // 화면에는 없지만 영속되는 키 — 창 위치·크기 기억.
    "ui.win_x",
    "ui.win_y",
    "ui.win_w",
    "ui.win_h",
    // 자동 실행 등록 마커(OS 등록 부재를 "외부 삭제"로 판정하는 기준).
    "app.autostart_reg",
    // 동기화 신원 표시 이름(비밀 값은 별도 보안 저장).
    "sync.handle",
    // ★ 09-02 창 상태 키 — 등재 누락으로 재시작 때 유실되던 것(09-02 발견):
    //   `set_by_name`은 아는 키만 state에 복원한다(§F-1). 팝업 크기 기억 ·
    //   미리보기 접힘 · 최상위 고정이 전부 조용히 기본값으로 돌아갔다.
    "ui.popup_w",
    "ui.popup_h",
    "ui.preview_open",
    "ui.dedup_view",
    "ui.always_on_top",
    // ★ 09-05 설정 창 위치·크기·모니터(09-03 settings_win이 쓰기 시작) — 등재 누락으로 ① 재시작 때
    //   복원이 안 되고(빈 값 → 기본 위치) ② 파일에 **같은 키가 두 번**(미지 키 보존분 + 런타임 known분) 남았다.
    "ui.set_x",
    "ui.set_y",
    "ui.set_w",
    "ui.set_h",
    "ui.set_mon",
    // ── 설정 체계 개편(10-10 · beep P2) ──
    // 고급 설정 스위치 상태(기본 off) — 설정 창 하단 스위치가 쓰고 열 때 되살린다.
    "ui.prefs_advanced",
    // 설정 검색 이력(탭 구분 · 최근 20) — 검색창 ↑/↓.
    "prefs.search",
];

/// ★ 설정 트리(10-10 · nexa-beep P2 · nexa-sql `CATEGORY_TREE` 차용 — DBeaver Preferences 모양): **그룹 → 카테고리**.
/// 사이드바·표시 순서·검색 결과 정렬의 단일 원천. 카테고리 안 하위 그룹(`Entry::sub`)은 본문 섹션 제목으로만 쓴다.
/// 새 카테고리는 여기 한 줄 — 트리에 없는 카테고리의 Entry는 시험 `tree_covers_every_category`가 잡는다.
pub const CATEGORY_TREE: &[(Msg, &[Msg])] = &[
    (Msg::GrpGeneral, &[Msg::CatGeneral, Msg::CatShortcuts]),
    (
        Msg::GrpClipboard,
        &[
            Msg::CatCapture,
            Msg::CatPaste,
            Msg::CatSearch,
            Msg::CatStorage,
            Msg::CatPrivacy,
        ],
    ),
    (Msg::GrpAppearance, &[Msg::CatAppearance]),
    (Msg::GrpSync, &[Msg::CatSync]),
    (Msg::GrpAdvanced, &[Msg::CatAdvanced, Msg::CatAbout]),
];

/// 트리 안 위치 `(그룹 순서, 카테고리 순서)` — 없으면 맨 뒤.
#[must_use]
pub fn tree_pos(cat: Msg) -> (usize, usize) {
    for (gi, (_, cats)) in CATEGORY_TREE.iter().enumerate() {
        if let Some(ci) = cats.iter().position(|c| *c == cat) {
            return (gi, ci);
        }
    }
    (usize::MAX, usize::MAX)
}

/// 카테고리가 속한 그룹.
#[must_use]
pub fn group_of(cat: Msg) -> Option<Msg> {
    CATEGORY_TREE
        .iter()
        .find(|(_, cats)| cats.contains(&cat))
        .map(|(g, _)| *g)
}

/// ★ 고급 설정(10-10 · 설정 창 "고급 설정" 스위치 대상) — 한 번 정하면 거의 손대지 않는 구현값·상한.
/// 글꼴·모드·켜기/끄기 같은 습관값은 기본 표시로 남긴다. 표에 없는 키는 시험 `advanced_keys_exist`가 잡는다.
pub const ADVANCED: &[&str] = &[
    // 표 캡처 그림 상한(셀 수·행·열) — 실측으로 정한 구현값.
    "cap.cell_pic_rows",
    "cap.cell_pic_cols",
    "cap.cell_pic_cells",
    // 네이티브 포맷 보관.
    "cap.native_formats",
    // 파일 동기화 상한·대역.
    "sync.files_max",
    "sync.file_bg_kbps",
    "sync.file_cache_mb",
    "sync.file_max_mb",
    // 진단 로그.
    "adv.log",
];

/// 고급 설정인가(설정 창 스위치가 꺼져 있으면 숨기고 수만 센다).
#[must_use]
pub fn is_advanced(key: &str) -> bool {
    ADVANCED.contains(&key)
}

/// ★ OS별 숨김(10-10 · 협업 세션 관찰 ⓒ "Windows에서도 Dock 아이콘 카드") — (키, 보이는 OS). 값은 영속·등재 그대로(다른 OS에서 열어도 잃지 않는다) · 화면·검색에서만 뺀다.
pub const SHOWN_ONLY_ON: &[(&str, &str)] = &[("ui.dock_icon", "macos")];

/// 이 OS에서 숨기는 키인가.
#[must_use]
pub fn hidden_on_this_os(key: &str) -> bool {
    SHOWN_ONLY_ON
        .iter()
        .any(|(k, os)| *k == key && *os != std::env::consts::OS)
}

/// 종속 조건(10-10 · nexa-sql `Dep` 차용 — "종속 설정은 부모가 조건을 만족할 때만 만질 수 있다").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dep {
    /// 부모가 `on`.
    On,
    /// 부모가 이 값.
    Eq(&'static str),
}

impl Dep {
    /// 부모 값이 조건을 만족하는가.
    #[must_use]
    pub fn satisfied(self, parent_value: &str) -> bool {
        match self {
            Dep::On => parent_value == "on",
            Dep::Eq(v) => parent_value == v,
        }
    }
}

/// (자식, 부모, 조건) — 부모가 조건을 만족하지 않으면 자식 행은 **잠긴다**(값은 유지 · 흐리게 + 안내 1줄).
/// 호스트가 `set_disabled`로 거는 런타임 잠금(동기화 켬/연결 상태 — settings_win)과 **합집합**이다.
pub const DEPENDS: &[(&str, &str, Dep)] = &[
    ("sync.files_max", "sync.files", Dep::On),
    ("sync.file_auto_mb", "sync.file_contents", Dep::On),
    ("sync.file_bg_kbps", "sync.file_contents", Dep::On),
    ("sync.file_max_mb", "sync.file_contents", Dep::On),
    ("sync.file_cache_mb", "sync.file_contents", Dep::On),
    ("sync.file_dir", "sync.file_contents", Dep::On),
    ("cap.cell_pic_rows", "cap.rich", Dep::On),
    ("cap.cell_pic_cols", "cap.rich", Dep::On),
    ("cap.cell_pic_cells", "cap.rich", Dep::On),
];

/// 이 키의 종속(부모 키, 조건) — 없으면 `None`.
#[must_use]
pub fn depends_of(key: &str) -> Option<(&'static str, Dep)> {
    DEPENDS
        .iter()
        .find(|(c, _, _)| *c == key)
        .map(|(_, p, d)| (*p, *d))
}

/// ★ 표시 순서(10-10 · nexa-sql `display_order` 차용) = (그룹, 카테고리, 하위 섹션, 키 접두 첫 등재, 등재 순).
/// 같은 카테고리에 여러 접두(`ui.`·`app.`)가 섞여도 접두끼리 모인다.
#[must_use]
pub fn display_order(idx: usize) -> (usize, usize, usize, usize, usize) {
    let e = &registry()[idx];
    let (g, c) = tree_pos(e.cat);
    // 하위 섹션: 직속(None) = 0 · 하위는 같은 카테고리 안 첫 등재 순.
    let si = match e.sub {
        None => 0,
        Some(sub) => {
            let mut seen: Vec<Msg> = Vec::new();
            for x in registry().iter().filter(|x| x.cat == e.cat) {
                if let Some(s) = x.sub {
                    if !seen.contains(&s) {
                        seen.push(s);
                    }
                }
            }
            seen.iter()
                .position(|y| *y == sub)
                .map_or(usize::MAX, |p| p + 1)
        }
    };
    let prefix = e.key.split('.').next().unwrap_or(e.key);
    let first = registry()
        .iter()
        .position(|x| {
            x.cat == e.cat && x.sub == e.sub && x.key.split('.').next().unwrap_or(x.key) == prefix
        })
        .unwrap_or(idx);
    (g, c, si, first, idx)
}

/// 직접 입력이 **텍스트**인 RadioInput 키(08-22) — 기본은 숫자 전용(포트·ms·MiB).
/// 서버 주소는 도메인·IP를 받아야 해서 숫자 필터가 입력 자체를 막았다(실기).
const FREE_TEXT_KEYS: &[&str] = &[
    "sync.relay",
    // 차단 목록 — URL 접두·앱 이름을 `;` 구분으로 편집한다(08-28).
    "sec.conceal_urls",
    "sec.exclude_apps",
];

/// 기본 off 토글 — 프로필 공개(DR-22 **기본 전부 비노출** · 옵트인). 미등록 토글은 on.
// (beep M3-2d ①의 `ui.close_to_tray` 공통 off는 08-30 정정으로 여기서 빠졌다 — 아래 주석.)
const TOGGLE_DEFAULT_OFF: &[&str] = &[
    "paste.plain_default",
    // ★ `ui.close_to_tray`는 **기본 켜짐으로 정정**(08-30 사용자 QA — Linux 실기 "닫기를 눌렀더니
    //   종료돼 다시 실행해야 함"). 클립보드 매니저는 24시간 상주가 존재 이유라 beep(대화 앱)의
    //   공통 off 확정을 그대로 물려받은 것이 틀렸다. 끄는 것은 설정에서 옵트아웃.
    "sec.clear_on_quit",
    // ★ D-79 브라우저 암호 차단 — 기본 꺼짐(사용자 확정 08-27 · 켜는 것은 옵트인).
    "sec.conceal_browser_pw",
    "adv.log",
    "sync.enabled",
];

/// Radio 기본값 예외 — 표시 순서(오름차순 등)와 기본값이 다른 키만 등록.
/// 미등록 키의 기본은 첫 옵션(기존 규약).
/// ★ 암호 미리보기 아이콘(09-03 사용자 지정 — Material `password visibility` 96² 알파).
const PW_EYE_ALPHA: &[u8] = include_bytes!("../assets/icon-pw-eye-96.alpha");
/// ★ 비밀번호 생성 아이콘(09-03 사용자 지정 — Material `flip_camera_android` 96² 알파).
const PW_REGEN_ALPHA: &[u8] = include_bytes!("../assets/icon-pw-regen-96.alpha");
/// 자산 변 크기(px).
const PW_EYE_SIDE: u32 = 96;

/// ★ 비밀 행 버튼 자리(09-03 사용자 — "두 버튼은 좌측에") — 상자 왼쪽 바깥에
/// [생성][미리보기(눈)] 순서로, 버튼 크기 = 상자 높이 · 간격 = 높이/8(절반 간격).
/// 생성 버튼 무장 유지 시간(09-03 사용자 — "2초 내에 다시 누르지 않으면 원복").
const PW_ARM_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);

/// ★ 기기 목록 한 행(09-04) — 텍스트 + [승인|해제] + [삭제]. `me` 행은 버튼이 없다.
pub struct DevRow {
    hex: String,
    text: String,
    emph: bool,
    approved: bool,
    approve: Option<Button>,
    del: Option<Button>,
    /// 버튼 폭(라벨 길이 산정 · i18n) — 승인/해제 · 삭제.
    bw: i32,
    dw: i32,
    /// 배치 결과(행 위 y · 텍스트 가용 폭 · 줄 수).
    y: i32,
    text_w: i32,
    lines: i32,
}

/// 라벨 폭 추정(논리 px · ASCII 7 · 그 외 13) — 실측 없이 배치하는 설명 예약과 같은 방식.
fn est_text_px(text: &str, scale: f32) -> i32 {
    let logical: i32 = text
        .chars()
        .map(|c| if c.is_ascii() { 7 } else { 13 })
        .sum();
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let px = (logical as f32 * scale).round() as i32;
    px
}

/// 줄 수 추정(1..=3) — 버튼 앞에서 감긴다(사용자 09-04 "설명이 버튼에 가리지 않게").
fn est_lines(text: &str, avail: i32, scale: f32) -> i32 {
    let avail = avail.max(1);
    ((est_text_px(text, scale) + avail - 1) / avail).clamp(1, 3)
}

impl core::fmt::Debug for DevRow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DevRow")
            .field("hex", &self.hex)
            .finish_non_exhaustive()
    }
}

impl DevRow {
    fn tick(&mut self, now_ms: u64) -> bool {
        let a = self.approve.as_mut().is_some_and(|b| b.tick(now_ms));
        let d = self.del.as_mut().is_some_and(|b| b.tick(now_ms));
        a | d
    }
    fn on_event(&mut self, ev: &InputEvent, inv: &mut Invalidations) {
        if let Some(b) = self.approve.as_mut() {
            b.on_event(ev, inv);
        }
        if let Some(b) = self.del.as_mut() {
            b.on_event(ev, inv);
        }
    }
    fn set_focused(&mut self, p: Point) {
        if let Some(b) = self.approve.as_mut() {
            b.set_focused(b.bounds().contains(p));
        }
        if let Some(b) = self.del.as_mut() {
            b.set_focused(b.bounds().contains(p));
        }
    }
}

/// 호스트 값(`hex\tstate\ttext` 줄들) → 행들. state: `me`(버튼 없음) · `approved` · `pending`.
fn build_dev_rows(value: &str, scale: f32, lang: Lang) -> Vec<DevRow> {
    use nexa_ctl::controls::ButtonTone;
    value
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.splitn(3, '\t');
            let hex = it.next().unwrap_or("").to_string();
            let state = it.next().unwrap_or("");
            let text = it.next().unwrap_or("").to_string();
            let me = state == "me";
            let approved = state == "approved";
            // 작은 버튼(09-04 사용자) — Status 폰트 · 폭 = 라벨 + 여백(i18n).
            let mk = |label: &str, tone: ButtonTone| {
                let mut b = Button::new(label)
                    .with_tone(tone)
                    .with_font(FontSlot::Status);
                b.set_scale(scale);
                b
            };
            #[allow(clippy::cast_possible_truncation)]
            let bw_of = |label: &str| est_text_px(label, scale) + (18.0 * scale).round() as i32;
            let (bw, dw) = if me {
                (0, 0)
            } else {
                (
                    bw_of(if approved {
                        tr(lang, Msg::StSyncRevoke)
                    } else {
                        tr(lang, Msg::SyncApproveVerb)
                    }),
                    bw_of(tr(lang, Msg::StSyncDelete)),
                )
            };
            DevRow {
                approve: (!me).then(|| {
                    if approved {
                        mk(tr(lang, Msg::StSyncRevoke), ButtonTone::Default)
                    } else {
                        mk(tr(lang, Msg::SyncApproveVerb), ButtonTone::Safe)
                    }
                }),
                del: (!me).then(|| mk(tr(lang, Msg::StSyncDelete), ButtonTone::Danger)),
                emph: me || approved,
                approved,
                hex,
                text,
                bw,
                dw,
                y: 0,
                text_w: 0,
                lines: 1,
            }
        })
        .collect()
}

/// 기기 목록 버튼 높이 — 컨트롤 높이의 3/4(텍스트 표현에 문제 없는 최소).
fn dev_btn_h(ctl_h: i32) -> i32 {
    ctl_h * 3 / 4
}

/// 행 하나의 텍스트 가용 폭(버튼·간격 제외).
fn dev_text_w(r: &DevRow, rw: i32, pad: i32, g: i32) -> i32 {
    if r.bw == 0 && r.dw == 0 {
        rw - pad * 2
    } else {
        rw - pad * 2 - r.bw - r.dw - g * 2
    }
}

/// 기기 목록 행들의 총 높이(빈 목록 = 설명 1줄) — 줄바꿈 예약 포함.
fn dev_rows_h(
    rows: &[DevRow],
    rw: i32,
    pad: i32,
    g: i32,
    ctl_h: i32,
    desc_line_h: i32,
    scale: f32,
) -> i32 {
    if rows.is_empty() {
        return desc_line_h;
    }
    let bh = dev_btn_h(ctl_h);
    rows.iter()
        .map(|r| {
            let lines = est_lines(&r.text, dev_text_w(r, rw, pad, g), scale);
            (lines * desc_line_h).max(bh) + g
        })
        .sum()
}

/// 보고 행 줄 수(빈 값 = 설명 1줄).
fn report_lines(v: Option<&String>) -> i32 {
    let n = v.map_or(0, |s| s.lines().filter(|l| !l.trim().is_empty()).count());
    i32::try_from(n.max(1)).unwrap_or(i32::MAX)
}

/// 비밀 행 버튼 자리(09-03 사용자 확정 "두 버튼을 텍스트 우상단으로" → 10-10 카드 레이아웃: 컨트롤이 좌하단으로
/// 가면서 버튼은 **상자 오른쪽** 같은 줄에 \[생성\]\[눈\] 순 · 버튼 크기 = 상자 높이 · 간격 = 높이/8).
fn pw_btn_rects(b: Rect) -> (Rect, Rect) {
    let regen = Rect::new(b.right() + b.h / 8, b.y, b.h, b.h);
    let eye = Rect::new(regen.right() + b.h / 8, b.y, b.h, b.h);
    (eye, regen)
}

/// 96² 알파 자산을 잉크색으로 틴트해 캐시에 담는다(색이 같으면 재사용).
fn tint_icon(
    cell: &std::cell::RefCell<Option<(u32, nexa_ctl::theme::IconImage)>>,
    alpha: &[u8],
    ink: u32,
) {
    let mut cache = cell.borrow_mut();
    let stale = !matches!(cache.as_ref(), Some((c, _)) if *c == ink);
    if stale {
        let (r, g, b) = ((ink >> 16) as u8, (ink >> 8) as u8, ink as u8);
        let mut rgba = Vec::with_capacity(alpha.len() * 4);
        for &a in alpha {
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        *cache = Some((
            ink,
            nexa_ctl::theme::IconImage::from_rgba(PW_EYE_SIDE, PW_EYE_SIDE, rgba),
        ));
    }
}

/// 틴트된 아이콘을 버튼 자리에 그린다(안쪽 여백 = 높이/8).
fn draw_icon(
    cell: &std::cell::RefCell<Option<(u32, nexa_ctl::theme::IconImage)>>,
    r: Rect,
    ctx: &mut dyn DrawCtx,
) {
    if let Some((_, img)) = cell.borrow().as_ref() {
        let ins = r.h / 8;
        let dst = Rect::new(r.x + ins, r.y + ins, r.w - ins * 2, r.h - ins * 2);
        ctx.image_scaled(dst, img, r);
    }
}

const RADIO_DEFAULTS: &[(&str, &str)] = &[
    ("app.lang", "system"),
    // ★ 차단 출처 기본값 = 코어 기본 접두 목록(레지스트리 테스트가 동기화를 강제).
    (
        "sec.conceal_urls",
        "edge://settings/autofill/passwords; edge://wallet/; edge://settings/passwords; chrome://password-manager/; chrome://settings/passwords; brave://password-manager/; vivaldi://password-manager/",
    ),
    // ★ 팝업은 마우스 위치에서 열린다(DR-24 — 사용자 확정 필수).
    ("ui.popup_at", "cursor"),
    ("ui.view_mode", "compact"),
    // ★ 팝업은 리치가 기본(09-02 사용자 확정 — "기본은 1").
    ("ui.popup_view", "rich"),
    // ★ 동기화 서버(09-03) — beep 공식 릴레이·기본 포트(같은 서버 공유 · DP-1).
    ("sync.relay", "beepd.sosomlab.com"),
    ("sync.port", "47300"),
    // ★ 파일 경로 전파 상한(09-12) — 보통의 복사(몇 개~수십 개)는 그대로 가고,
    //   폴더 전체 선택 같은 폭주만 자른다. 후보는 오름차순이라 첫 후보가 기본이 아니다.
    ("sync.files_max", "1000"),
    // ★ 파일 내용 공유(09-12 · docs/26 D-65/D-67 조정) — 자동 캐시 50MB(문서·이미지 대부분) ·
    //   백그라운드 1MB/s · 붙여넣기 시 1000MB(실수로 GB급을 끌어오지 않게) · 캐시 2000MB.
    ("sync.file_auto_mb", "50"),
    ("sync.file_bg_kbps", "1024"),
    ("sync.file_max_mb", "1000"),
    ("sync.file_cache_mb", "2000"),
    ("ui.theme", "system"),
    ("ui.tray_recent_n", "8"),
    // ★ Excel 셀 범위 그림 상한(10-04) — nclip_core::capture::CELL_PICTURE_MAX_* 와 같은 값.
    ("cap.cell_pic_rows", "200"),
    ("cap.cell_pic_cols", "50"),
    ("cap.cell_pic_cells", "5000"),
    ("store.max_items", "1000"),
    // ★ T-13(09-01 확정): 기간 무제한(0) + 총용량 500MB.
    ("store.max_age_days", "0"),
    ("store.max_total_mb", "500"),
    ("store.sort", "recent"),
    ("find.mode", "fuzzy"), // ★ 기본 = 유사(09-04 사용자 — 띄어쓴 단어 전부 · 순서 무관)
];

/// 항목 종류 — 우측 패널이 이 열거를 읽어 컨트롤을 동적 생성한다(새 설정 = Entry 1줄).
#[derive(Clone, Copy, Debug)]
pub enum SettingKind {
    /// 값 후보 중 택일 — [`Combo`] 드롭다운.
    Radio(&'static [(&'static str, Msg)]),
    /// 택일 + **직접 입력** — 후보에 없는 값을 인라인 편집으로 넣는다(값, 표시 접미).
    RadioInput(&'static [(&'static str, Msg)], &'static str),
    /// ★ **숫자 항목** — 후보도 값도 **숫자 그대로 보인다**(사용자 확정 08-26).
    ///
    /// [`Radio`](SettingKind::Radio)와 달리 라벨을 [`Msg`]로 번역하지 않는다 —
    /// *"보통"* 이 몇 개인지 알려면 설명을 읽어야 하지만 **`1000`은 그 자체로 답**이다.
    /// 후보에 없는 값은 인라인 편집으로 직접 넣는다(숫자 필터 · 범위 검사는 [`validate`]).
    ///
    /// `presets`는 **적은 순서 그대로** 뜬다(오름차순으로 적을 것).
    Number {
        /// 빠른 선택용 값들 — 문자열이 **그대로 라벨**이 된다.
        presets: &'static [&'static str],
        /// 값 뒤에 붙는 단위(없으면 `""`).
        suffix: &'static str,
    },
    /// 3×3 위치 그리드 — 미니 화면(4:3) 셀로 직관 선택([`PositionPicker`]).
    PositionGrid,
    /// 글꼴 **얼굴만** — 크기는 Base UI를 따른다(고정폭 슬롯).
    FontFace {
        /// 글꼴명 값 키.
        family_key: &'static str,
    },
    /// on/off — [`Checkbox`](nexa_ctl::controls::Checkbox). 값은 `"on"`/`"off"`(기본 on).
    Toggle,
    /// 색상 — [`ColorPicker`](스와치 + `#RRGGBB` 입력 + 프리셋). 값 = `#RRGGBB`(08-10).
    Color {
        /// 기본 hex(테마 팔레트의 원값).
        default: &'static str,
    },
    /// 글꼴 영역 — 글꼴명 [`TextBox`] + 크기 [`Combo`].
    FontSection {
        /// 글꼴명 값 키(`font.{region}.family`).
        family_key: &'static str,
        /// 크기 값 키(`font.{region}.size`).
        size_key: &'static str,
    },
    /// ★ **문자열 목록**(08-31 사용자 요청 — 차단 페이지·제외 앱) — ListBox +
    /// 추가/삭제 + 행 인라인 편집([`ListEditor`]). 값 = `;` 구분 한 줄(저장 형식 불변).
    ListEdit,
    /// ★ **보고 행**(09-03 — 기기 목록) — 컨트롤 없이 호스트가 `set_value`로 채우는 읽기 전용
    /// 줄들(개행 구분 · 줄 앞 `*` = 강조). 비면 설명이 대신 보인다.
    Report,
    /// ★ **기기 목록**(09-04 — 행별 승인) — 호스트가 `set_value`로 `hex\tstate\ttext` 줄들을 주면
    /// 행마다 [승인|해제][삭제] 버튼을 붙인다(state = me|approved|pending · me는 버튼 없음).
    /// 클릭 = `(key, "approve:hex" | "revoke:hex" | "delete:hex")` 변경 방출.
    DeviceList,
    /// 실행 버튼 — 값이 아니라 **행위**(백업·복원 등). 클릭 = `(key, "run")` 변경 방출.
    /// 값 키가 없어(`default_values` 빈 목록) 영속 파일에 실리지 않는다.
    Action {
        /// 버튼 라벨.
        verb: Msg,
    },
    /// ★ 전역 단축키(09-04 사용자) — 값 = 조합 문자열(`Shift+Alt+C` · 빈 값 = 없음). 버튼 라벨이 곧 조합이고,
    /// 클릭 = `(key, "run")` 방출 → 호스트가 캡처 오버레이를 연다(제거·확인·취소).
    Hotkey {
        /// 기본 조합.
        default: &'static str,
        /// ★ 전역(OS 등록 · 수식 키 필수)인가 — 창 안 단축키는 `false`(09-08 · 맨 키 허용).
        global: bool,
    },
    /// ★ 읽기 전용 정보 행(10-10 · beep P2) — 값 키 없음 · 호스트가 `set_info`로 본문을 채운다(라이선스 상태 등).
    Info,
    /// ★ 자유 문자열 한 줄(09-03 동기화 기반 — 핸들·패스프레이즈·서버 주소).
    /// [`FontFace`](SettingKind::FontFace)의 TextBox 행(`RowCtl::Face`)을 재사용한다 —
    /// 플러시가 `e.key` 범용이라 추가 배선이 없다.
    Text {
        /// 빈 값일 때 안내(placeholder).
        hint: Msg,
        /// ★ 비밀 값(09-03) — ●로 가리고 우측 눈 버튼으로 보기 토글.
        secret: bool,
    },
}

/// 설정 항목(레지스트리 최소 단위).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// 카테고리(사이드바·검색 대상).
    pub cat: Msg,
    /// 제목(검색 대상 · 글꼴 섹션에선 섹션 제목).
    pub label: Msg,
    /// 회색 설명 한 줄(검색 대상).
    pub desc: Msg,
    /// 하위 카테고리(없으면 최상위 직속) — 사이드바 계층·필터 근거.
    pub sub: Option<Msg>,
    /// 컨트롤 형태.
    pub kind: SettingKind,
    /// 값 키(안정 계약 — rename 시 마이그레이션). 글꼴 섹션에선 `family_key`와 동일.
    pub key: &'static str,
}

impl Entry {
    /// 레지스트리 기본값(각 값 키 → 기본 문자열). **한 항목이 값 키를 여럿 가질 수
    /// 있다**(FontSection = family+size) — 초기화·시드가 `key` 하나만 돌면 짝 키가
    /// 새므로, 기본값을 다루는 쪽은 반드시 이 목록을 쓴다(08-15 초기화 점검).
    pub fn default_values(&self) -> Vec<(&'static str, String)> {
        match self.kind {
            // 숫자 항목 — 예외 표에 있으면 그 값, 없으면 첫 후보.
            SettingKind::Number { presets, .. } => RADIO_DEFAULTS
                .iter()
                .find(|(k, _)| *k == self.key)
                .map(|(_, v)| *v)
                .or_else(|| presets.first().copied())
                .map(|v| (self.key, v.to_string()))
                .into_iter()
                .collect(),
            SettingKind::Radio(opts) | SettingKind::RadioInput(opts, _) => RADIO_DEFAULTS
                .iter()
                .find(|(k, _)| *k == self.key)
                .map(|(_, v)| *v)
                .or_else(|| opts.first().map(|(v, _)| *v))
                .map(|v| (self.key, v.to_string()))
                // ★ 옵션 없는 자유 입력형(RadioInput(&[], _) — net.server.address)도
                //   **빈 기본값으로 키를 등록**한다. 키가 values 맵에 없으면
                //   set_by_name이 "미지 키"로 흘려 — 화면에서 입력·저장한 값이
                //   재시작 때마다 조용히 증발했다(08-22 X-2b 배선이 발각 · 08-14
                //   "저장은 되는데 로드가 무시" 영속 구멍과 같은 유형).
                .or_else(|| {
                    matches!(self.kind, SettingKind::RadioInput(..))
                        .then(|| (self.key, String::new()))
                })
                .into_iter()
                .collect(),
            // 목록 — 기본 표(차단 URL 기본 목록)에 있으면 그 값, 없으면 **빈 값으로 키 등록**
            // (RadioInput과 같은 이유 — 키가 없으면 저장값이 미지 키로 증발한다 · 08-22).
            // 보고 행 — 키는 등록(호스트 set_value 대상), 값은 비어 시작.
            SettingKind::Report | SettingKind::DeviceList => vec![(self.key, String::new())],
            SettingKind::ListEdit => vec![(
                self.key,
                RADIO_DEFAULTS
                    .iter()
                    .find(|(k, _)| *k == self.key)
                    .map(|(_, v)| (*v).to_string())
                    .unwrap_or_default(),
            )],
            SettingKind::Toggle => {
                // 프로필 공개는 **기본 비노출**(DR-22 — 옵트인). 그 외 토글은 기본 on.
                let on = !TOGGLE_DEFAULT_OFF.contains(&self.key);
                vec![(self.key, if on { "on" } else { "off" }.to_string())]
            }
            SettingKind::Color { default } => vec![(self.key, default.to_string())],
            SettingKind::PositionGrid => vec![(self.key, "bl".to_string())],
            SettingKind::FontFace { family_key } => vec![(family_key, String::new())],
            SettingKind::Text { .. } => vec![(self.key, String::new())],
            // 정보 행 — 값 키 없음(영속 파일에 실리지 않는다 · 호스트가 set_info로 채운다).
            SettingKind::Info => Vec::new(),
            SettingKind::FontSection {
                family_key,
                size_key,
            } => vec![
                (family_key, String::new()), // 빈 문자열 = 시스템 기본 글꼴
                (size_key, FONT_SIZE_DEFAULT.to_string()),
            ],
            // 행위 항목은 값이 없다 — 영속·검증 대상에서 자연히 빠진다.
            SettingKind::Action { .. } => vec![],
            SettingKind::Hotkey { default, .. } => vec![(self.key, default.to_string())],
        }
    }
}

/// 단축키 버튼 라벨 — 조합 표시(mac은 기호) · 빈 값 = "없음".
fn hotkey_label(value: &str, lang: Lang) -> String {
    match nclip_core::hotkey::Hotkey::parse(value) {
        Some(h) => h.display(cfg!(target_os = "macos")),
        None => tr(lang, Msg::HotkeyNone).to_string(),
    }
}

/// 입력 검증 규칙(08-20 — 확정 시 즉시 검사): `Ok` = 적용 · `Err(경고 Msg)` =
/// 호스트가 경고를 띄우고 컨트롤은 **직전 확정값으로 원복**([`Control::last_value`]).
/// 새 규칙은 여기 한 곳에만 추가한다(검증·경고·원복 배선은 공용).
fn validate(key: &str, value: &str) -> Result<(), Msg> {
    /// 숫자 + 범위 — 어긋나면 호스트가 경고하고 **직전 확정값으로 원복**한다.
    fn range(value: &str, lo: u64, hi: u64, err: Msg) -> Result<(), Msg> {
        if value.parse::<u64>().is_ok_and(|v| (lo..=hi).contains(&v)) {
            Ok(())
        } else {
            Err(err)
        }
    }
    match key {
        // ★ 보관 개수(사용자 확정 08-26 — 숫자 직접 입력).
        //   하한 10 = 그보다 적으면 히스토리라고 할 게 없다.
        //   상한 100000 = 24시간 상주 앱의 메모리·검색 예산([docs/00 §2] 검색 ≤16ms).
        "store.max_items" => range(value, 10, 100_000, Msg::ValItemsRange),
        // 트레이 최근 개수 — 메뉴가 화면 밖으로 넘지 않는 선.
        "ui.tray_recent_n" => range(value, 3, 20, Msg::ValTrayCountRange),
        // 정지 방치 자동 취소 — 1~10분 범위(사용자 확정 08-20).
        "xfer.auto_cancel_min" => {
            if value.parse::<u64>().is_ok_and(|v| (1..=10).contains(&v)) {
                Ok(())
            } else {
                Err(Msg::ValMinutesRange)
            }
        }
        _ => Ok(()),
    }
}

/// 설정 레지스트리 — **실존 설정만**. 렌더·검색·기본값이 전부 여기서 나온다.
#[must_use]
pub fn registry() -> &'static [Entry] {
    crate::settings_registry::REGISTRY
}

/// 설정 값 저장(런타임) — 영속은 M2-5의 `Repository` 포트로 감싼다.
#[derive(Debug, Default)]
pub struct SettingsState {
    values: HashMap<&'static str, String>,
}

impl SettingsState {
    /// 레지스트리 기본값으로 초기화.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut values = HashMap::new();
        for e in registry() {
            for (k, v) in e.default_values() {
                values.insert(k, v);
            }
        }
        for k in HIDDEN_KEYS {
            values.insert(*k, String::new());
        }
        Self { values }
    }

    /// 현재 값(미설정 키는 빈 문자열).
    #[must_use]
    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    /// 값 지정.
    pub fn set(&mut self, key: &'static str, value: String) {
        self.values.insert(key, value);
    }

    /// 저장 스냅샷 — 전체 (키, 값) 쌍을 **키 정렬**로(직렬화가 결정적이어야
    /// "직전 저장분과 같으면 쓰지 않는다"(ADR-0011 S-3) 비교가 성립한다).
    #[must_use]
    pub fn known_pairs(&self) -> Vec<(&'static str, &str)> {
        let mut pairs: Vec<_> = self.values.iter().map(|(k, v)| (*k, v.as_str())).collect();
        pairs.sort_unstable_by_key(|(k, _)| *k);
        pairs
    }

    /// 파일에서 읽은 (키, 값)을 적용한다 — **아는 키만**, 값은 Kind별 관용 검증
    /// (ADR-0011 §4-3: 거부·실패가 아니라 무시 = 기본값 유지). 반환 = 아는 키였는가
    /// (거짓이면 호출자가 미지 키로 보존한다 — F-1).
    pub fn set_by_name(&mut self, key: &str, value: &str) -> bool {
        // 화면 밖 영속 키(M3-17 프로필 필드) — 자유 문자열 그대로.
        if let Some(k) = HIDDEN_KEYS.iter().find(|k| **k == key) {
            self.values.insert(k, value.to_string());
            return true;
        }
        // &'static str 키는 레지스트리에서 찾는다(default_values가 파생 키 포함 전부).
        let mut found: Option<&'static str> = None;
        let mut kind: Option<SettingKind> = None;
        'outer: for e in registry() {
            for (k, _) in e.default_values() {
                if k == key {
                    found = Some(k);
                    // FontSection의 size 파생 키는 Radio류가 아니므로 kind 검증에서
                    // family/size를 구분한다 — 아래 검증 참조.
                    kind = Some(e.kind);
                    break 'outer;
                }
            }
        }
        let (Some(k), Some(kind)) = (found, kind) else {
            return false;
        };
        let valid = match kind {
            SettingKind::Radio(opts) => opts.iter().any(|(v, _)| *v == value),
            // 직접 입력 허용 — 빈 값만 거른다(빈 문자열은 기본값 의미가 아니다).
            SettingKind::RadioInput(..) => !value.is_empty(),
            // 목록은 빈 것도 유효(제외 앱 기본 = 비어 있음).
            SettingKind::ListEdit | SettingKind::Report | SettingKind::DeviceList => true,
            // ★ 숫자 항목 — 파일에서 온 값도 **숫자여야** 받는다. 범위는 validate가 본다.
            SettingKind::Number { .. } => value.parse::<u64>().is_ok(),
            SettingKind::Toggle => value == "on" || value == "off",
            SettingKind::Color { .. } => nexa_ctl::theme::color_from_hex(value).is_some(),
            // 위치 코드·글꼴명(빈 값 = 시스템 기본)·크기 코드는 소비처가 관용 파싱한다.
            SettingKind::PositionGrid | SettingKind::FontFace { .. } => true,
            SettingKind::Text { .. } => true,
            SettingKind::FontSection { .. } => true,
            // 행위 항목은 값이 없다 — 파일에서 와도 무시(default_values가 비어 도달 불가).
            SettingKind::Action { .. } | SettingKind::Info => false,
            // 단축키 = 빈 값(없음) 또는 파싱되는 조합만.
            SettingKind::Hotkey { .. } => {
                value.trim().is_empty() || nclip_core::hotkey::Hotkey::parse(value).is_some()
            }
        };
        if valid {
            self.values.insert(k, value.to_string());
        }
        true // 아는 키다 — 값이 무효여도 미지 키로 보존하지 않는다(기본값 유지).
    }
}

/// 검색어 → 소문자 토큰(공백 구분 **AND 매칭** — VS Code 규약).
fn tokens(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_lowercase).collect()
}

/// 설명 워드랩(08-11) — `avail`(물리 px) 안에서 그리디 줄바꿈. 공백 없는 긴 조각
/// (CJK 문장 등)은 문자 단위로 쪼갠다. `max_lines` 초과분은 마지막 줄 끝을 `…`로 접는다
/// (예약 줄 수는 레이아웃의 추정 — 실측이 넘치면 자르는 쪽이 침범보다 낫다).
pub(crate) fn wrap_text(
    ctx: &mut dyn DrawCtx,
    text: &str,
    avail: i32,
    max_lines: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        let cand = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if ctx.text_width(&cand) <= avail {
            cur = cand;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if ctx.text_width(word) > avail {
            for ch in word.chars() {
                let cand = format!("{cur}{ch}");
                if cur.is_empty() || ctx.text_width(&cand) <= avail {
                    cur = cand;
                } else {
                    lines.push(std::mem::take(&mut cur));
                    cur = ch.to_string();
                }
            }
        } else {
            cur = word.to_string();
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines.max(1));
        if let Some(last) = lines.last_mut() {
            while !last.is_empty() && ctx.text_width(&format!("{last}…")) > avail {
                last.pop();
            }
            last.push('…');
        }
    }
    lines
}

/// 검색 매칭(10-10 개정 · beep D-33-1): 공백 토큰 **AND** · 대상 = 키 이름 + 그룹·카테고리·하위·제목·설명(전 언어) ·
/// 한글 토큰은 **자모열 대조**(조합 중 "ㅌ"·"테"도 "테마"에 맞는다 · [`crate::jamo`]).
/// 전 언어에 걸쳐 매칭한다 — 영어 UI에서도 "테마"로, 한국어 UI에서도 "theme"로 찾힌다.
fn entry_matches(e: &Entry, toks: &[String]) -> bool {
    if toks.is_empty() {
        return true;
    }
    let mut hay = String::new();
    hay.push_str(e.key);
    hay.push(' ');
    for lang in Lang::ALL {
        if let Some(g) = group_of(e.cat) {
            hay.push_str(tr(lang, g));
            hay.push(' ');
        }
        hay.push_str(tr(lang, e.cat));
        hay.push(' ');
        if let Some(sub) = e.sub {
            hay.push_str(tr(lang, sub));
            hay.push(' ');
        }
        hay.push_str(tr(lang, e.label));
        hay.push(' ');
        hay.push_str(tr(lang, e.desc));
        hay.push(' ');
    }
    let hay = hay.to_lowercase();
    toks.iter().all(|t| {
        if crate::jamo::has_hangul(t) {
            crate::jamo::contains_jamo(&hay, &crate::jamo::decompose(t, true), true)
        } else {
            hay.contains(t.as_str())
        }
    })
}

// 레이아웃(논리 px).
const SIDEBAR_W: i32 = 170;
const SEARCH_H: i32 = 30;
/// 하단 줄(고급 스위치 · 설정 파일 열기 · 닫기 — 10-10 beep P2 · nexa-sql 차용) 높이.
const BOTTOM_H: i32 = 44;
/// 고급 숨김 배너(밴드 셋째 줄) 높이.
const BANNER_H: i32 = 22;
/// 카드 \[초기화\] 버튼 폭.
const RESET_W: i32 = 72;
/// 검색 이력 보관 수.
const HISTORY_MAX: usize = 20;
/// 카드 제목 줄 높이.
const TITLE_H: i32 = 20;
/// 카드 안쪽 여백 · 카드 사이 간격.
const CARD_PAD: i32 = 12;
const CARD_GAP: i32 = 8;
/// 자유 문자열 상자 폭.
const TEXT_W: i32 = 260;
/// 설정 행에 붙는 정보 줄 높이(논리 px).
const NOTE_H: i32 = 22;
/// 설명 워드랩 줄 높이(논리 px — Status 폰트 한 줄 + 행간).
const DESC_LINE_H: i32 = 16;
const CTL_H: i32 = 26;
const COMBO_W: i32 = 170;
const SIZE_W: i32 = 112;
const FAMILY_W: i32 = 180;
const PAD: i32 = 12;
/// 스크롤 영역 안의 하위 섹션 제목 높이 — **위쪽 여백을 크게** 둬서 앞 그룹과 확실히 끊는다
/// (사용자 지적 08-11: 그룹 경계가 눈에 잘 안 띈다). 제목 글자는 이 상자의 **아래쪽**에 붙는다.
const SUB_HEAD_H: i32 = 52;
/// 하위 제목 상자에서 글자 아래 여백 — 제목이 자기 그룹 첫 행에 가깝게 붙게 한다.
const SUB_HEAD_PAD_B: i32 = 8;
/// 상단 고정 밴드 — 상위 제목 줄 + 하위 제목 줄(하위가 없으면 아랫줄은 비워 둔다).
/// **높이를 고정**해야 그룹을 넘나들 때 내용이 위아래로 튀지 않는다.
const CRUMB_CAT_H: i32 = 30;
const CRUMB_SUB_H: i32 = 24;

/// 우측 한 행 = 레지스트리 항목 + 실물 컨트롤.
#[derive(Debug)]
enum RowCtl {
    Combo(Combo),
    /// on/off 토글 — mac(iOS) 스타일 [`Switch`](08-11 · 기존 Checkbox에서 교체).
    Check(Switch),
    /// 실행 버튼(백업·복원 등 행위 항목).
    Act(Button),
    Font {
        family: Box<TextBox>,
        size: Combo,
    },
    /// 3×3 위치 그리드.
    Pos(PositionPicker),
    /// ★ 문자열 목록(ListBox + 추가/삭제 + 인라인 편집) — 큼직해 Box(변형 크기 격차 린트).
    List(Box<ListEditor>),
    /// ★ 보고 행(09-03) — 컨트롤 없음(줄들은 values에서 읽어 그린다).
    Report,
    /// ★ 기기 목록(09-04) — 행별 버튼.
    Devices(Vec<DevRow>),
    /// 글꼴 **얼굴만**(고정폭 — 크기는 Base UI를 따른다).
    Face(TextBox),
    /// 색상(스와치 + hex + 프리셋 · 08-10).
    Color(ColorPicker),
    /// 읽기 전용 정보(10-10 · 호스트가 채운 글).
    Info(String),
}

#[derive(Debug)]
struct RowUi {
    /// registry 인덱스.
    idx: usize,
    /// 행 영역(우측 패널 안 · 물리 px).
    rect: Rect,
    ctl: RowCtl,
    /// 이 행이 속한 그룹 `(상위, 하위)` — 상단 고정 밴드가 무엇을 보여줄지 정한다.
    group: (Msg, Option<Msg>),
    /// 이 행 **위에** 그릴 하위 섹션 제목(그룹의 첫 행에만). 상위 직속 구간은 `None`
    /// (상위 제목은 스크롤되지 않는 밴드가 늘 보여주므로 본문에 또 적지 않는다).
    head: Option<Msg>,
    /// 헤더까지 포함한 이 행의 시작 y(레이아웃이 채운다) — 밴드 판정에 쓴다.
    head_h: i32,
    /// 설명에 예약된 줄 수(1~3 · 레이아웃이 추정) — 워드랩이 이 안에서 그린다(08-11).
    desc_lines: i32,
    /// 설명 워드랩 가용 폭(물리 px — 컨트롤 왼쪽까지). 레이아웃·페인트가 같은 값을 쓴다.
    desc_avail: i32,
    /// [초기화](10-10 · 값이 기본값과 다를 때만 보인다 · 값 키가 없는 행위/정보/보고 행은 `None`).
    reset: Option<Button>,
    /// 키 이름+복사 글리프 자리(페인트가 실측해 채운다 · 클릭 = 키 복사 요청).
    key_rect: std::cell::Cell<Rect>,
}
/// 사이드바 트리 선택(10-10 그룹 트리) — 그룹 행 = 그 그룹의 카테고리 전부 · 카테고리 행 = 그것만.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeSel {
    /// `CATEGORY_TREE` 그룹 인덱스.
    Group(usize),
    /// `SettingsWidget::cats` 인덱스(트리 순서의 카테고리).
    Cat(usize),
}
/// 설정 위젯 — 커스텀 컨트롤 컴포지션.
#[derive(Debug)]
pub struct SettingsWidget {
    bounds: Rect,
    scale: f32,
    /// 검색 입력(TextBox).
    search: TextBox,
    /// 검색어 미러(rebuild 트리거 비교용).
    query: String,
    /// 시스템 기본 폰트 표시 이름(placeholder 식별 — 비면 이름 생략).
    default_base_name: String,
    /// 시스템 고정폭 폰트 표시 이름.
    default_mono_name: String,
    /// 카테고리 사이드바(TreeView).
    tree: TreeView,
    /// 사이드바 가시 행 → 트리 선택(그룹/카테고리 · 트리 행 순서와 같다).
    cat_map: Vec<TreeSel>,
    /// 현재 선택.
    selected: TreeSel,
    /// 고급 설정 스위치 상태(`ui.prefs_advanced`) — 끄면 [`ADVANCED`] 항목은 숨기고 수만 센다.
    advanced: bool,
    /// 지금 보기에서 숨긴 고급 항목 수(밴드 배너).
    adv_hidden: usize,
    /// 하단 줄 컨트롤 — 고급 스위치 · \[설정 파일 열기…\] · \[닫기\].
    adv_switch: Switch,
    btn_file: Button,
    btn_close: Button,
    /// 읽기 전용 정보 행 본문(`SettingKind::Info` — 호스트 `set_info`).
    infos: HashMap<&'static str, String>,
    /// 검색 이력(최근이 앞 · `prefs.search` 탭 구분 · 최대 [`HISTORY_MAX`]) · ↑/↓ 탐색 위치.
    history: Vec<String>,
    hist_pos: Option<usize>,
    /// 우측 행들(가시 항목 + 컨트롤).
    rows: Vec<RowUi>,
    /// 현재 값 스냅숏(컨트롤 초기화·보고 근거).
    values: HashMap<&'static str, String>,
    changes: Vec<(&'static str, String)>,
    /// 검증 실패 경고(08-20 — 확정 즉시 · 호스트가 모달/상태줄로 표출).
    warnings: Vec<Msg>,
    back: bool,
    /// 마지막 마우스 위치(휠 라우팅용).
    last_mouse: Point,
    /// 우측 패널 세로 스크롤 오프셋(물리 px).
    scroll: i32,
    /// 우측 패널 콘텐츠 총 높이(물리 px) — layout에서 계산.
    content_h: i32,
    /// 우측 패널 오버레이 스크롤바.
    bars: ScrollBars,
    /// 사이드바 폭(논리 px) — 스플리터 드래그로 조절(사용자 요청 08-09).
    sidebar_w: i32,
    /// 스플리터 드래그 중.
    split_drag: bool,
    /// ★ 스플리터 위에 커서가 있는가 — VS Code처럼 **하이라이트**로 조절 가능함을 알린다.
    split_hover: bool,
    /// ★ 글로우 — hover하면 **서서히 밝아진다**(사용자 요청 08-26).
    ///
    /// 손으로 보간하던 것을 공용 [`Fade`]로 바꿨다(08-26 2차) — hover 페이드를
    /// 트리·버튼·콤보로 넓히면서 **같은 타이밍을 세 번 적지 않기 위해**서다.
    split_fade: Fade,
    /// 비활성 설정 키(호스트가 지정) — 흐리게 그리고 입력을 받지 않는다.
    disabled: std::collections::HashSet<&'static str>,
    /// 특정 설정 행 **바로 아래**에 붙는 한 줄 정보(자리 고정 — 호스트가 채운다).
    notes: HashMap<&'static str, (String, NoteTone)>,
    /// 암호 눈 아이콘 틴트 캐시(색 키 — 96² 재틴트 방지).
    pw_eye: std::cell::RefCell<Option<(u32, nexa_ctl::theme::IconImage)>>,
    /// 비밀번호 생성 아이콘 틴트 캐시(색 키).
    pw_regen: std::cell::RefCell<Option<(u32, nexa_ctl::theme::IconImage)>>,
    /// ★ 생성 2단 확인(09-03 사용자) — 첫 클릭 = 무장(빨강) · 2초 안 재클릭 = 생성 ·
    ///   지나면 원복. 호스트 시계에 안 기대고 자체 Instant(유휴 뒤 첫 클릭 오판 방지).
    pw_arm: Option<std::time::Instant>,
}

/// 행 노트의 시각 톤(08-22 — "검증됨"이 눈에 띄어야 한다는 사용자 요청).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteTone {
    /// 일반 정보(종전 동작 — 흐린 글자·배경 없음).
    Plain,
    /// 긍정 상태(검증 완료 등) — ok색 옅은 배경 + ok색 글자.
    Ok,
    /// 경고 상태 — warn색 옅은 배경 + warn색 글자.
    Warn,
    /// 자동 판정 정보(08-22 — 서버 타입 판정·관측값처럼 **시스템이 정한 값**을
    /// 눈에 띄게) — accent색 옅은 배경 + 본문색 글자.
    Info,
}

impl SettingsWidget {
    /// 현재 값 스냅숏으로 연다.
    #[must_use]
    pub fn new(state: &SettingsState) -> Self {
        let mut values = HashMap::new();
        for e in registry() {
            for (k, _) in e.default_values() {
                values.insert(k, state.get(k).to_string());
            }
        }
        let lang = current_lang();
        let advanced = state.get("ui.prefs_advanced") == "on";
        let history: Vec<String> = state
            .get("prefs.search")
            .split('\t')
            .filter(|h| !h.is_empty())
            .take(HISTORY_MAX)
            .map(str::to_string)
            .collect();
        let mut w = Self {
            bounds: Rect::default(),
            scale: 1.0,
            search: TextBox::new("Search").with_clearable(),
            query: String::new(),
            default_base_name: String::new(),
            default_mono_name: String::new(),
            tree: TreeView::new(TreeModel::default()),
            cat_map: Vec::new(),
            selected: TreeSel::Cat(0),
            advanced,
            adv_hidden: 0,
            adv_switch: Switch::new(tr(lang, Msg::PrefsAdvanced), advanced),
            btn_file: Button::new(tr(lang, Msg::BtnOpenSettingsFile)),
            btn_close: Button::new(tr(lang, Msg::BtnClose)),
            infos: HashMap::new(),
            history,
            hist_pos: None,
            rows: Vec::new(),
            values,
            changes: Vec::new(),
            warnings: Vec::new(),
            back: false,
            last_mouse: Point { x: -1, y: -1 },
            scroll: 0,
            content_h: 0,
            bars: ScrollBars::new(),
            sidebar_w: SIDEBAR_W,
            split_drag: false,
            split_hover: false,
            split_fade: Fade::hover(),
            disabled: std::collections::HashSet::new(),
            notes: HashMap::new(),
            pw_eye: std::cell::RefCell::new(None),
            pw_regen: std::cell::RefCell::new(None),
            pw_arm: None,
        };
        let mut inv = Invalidations::default();
        w.rebuild(&mut inv);
        w
    }

    /// 선택 복사(① 08-13) — 포커스된 텍스트 입력(검색·글꼴명)에서만 나온다.
    #[must_use]
    pub fn clipboard_copy(&self) -> Option<String> {
        if let Some(t) = self.search.copy_selection() {
            return Some(t);
        }
        // 콤보 직접 입력(08-22 — TextBox 위임)도 같은 텍스트 입력이다.
        self.rows.iter().find_map(|r| match &r.ctl {
            RowCtl::Font { family, size } => family
                .copy_selection()
                .or_else(|| size.editing_input_ref().and_then(TextBox::copy_selection)),
            RowCtl::Face(family) => family.copy_selection(),
            RowCtl::Combo(c) => c.editing_input_ref().and_then(TextBox::copy_selection),
            RowCtl::List(l) => l.editing_input_ref().and_then(TextBox::copy_selection),
            _ => None,
        })
    }

    /// 선택 잘라내기(①).
    pub fn clipboard_cut(&mut self, inv: &mut Invalidations) -> Option<String> {
        if let Some(t) = self.search.cut_selection(inv) {
            self.sync_query(inv);
            return Some(t);
        }
        let got = self.rows.iter_mut().find_map(|r| match &mut r.ctl {
            RowCtl::Font { family, size } => family
                .cut_selection(inv)
                .or_else(|| size.editing_input().and_then(|tb| tb.cut_selection(inv))),
            RowCtl::Face(family) => family.cut_selection(inv),
            RowCtl::Combo(c) => c.editing_input().and_then(|tb| tb.cut_selection(inv)),
            RowCtl::List(l) => l.editing_input().and_then(|tb| tb.cut_selection(inv)),
            _ => None,
        });
        self.drain_changes(inv);
        got
    }

    /// 붙여넣기(①) — 포커스된 텍스트 입력만 받는다.
    pub fn clipboard_paste(&mut self, text: &str, inv: &mut Invalidations) {
        self.search.paste(text, inv);
        self.sync_query(inv);
        for r in &mut self.rows {
            match &mut r.ctl {
                RowCtl::Font { family, size } => {
                    family.paste(text, inv);
                    if let Some(tb) = size.editing_input() {
                        tb.paste(text, inv);
                    }
                }
                RowCtl::Face(family) => family.paste(text, inv),
                RowCtl::Combo(c) => {
                    if let Some(tb) = c.editing_input() {
                        tb.paste(text, inv);
                    }
                }
                RowCtl::List(l) => {
                    if let Some(tb) = l.editing_input() {
                        tb.paste(text, inv);
                    }
                }
                _ => {}
            }
        }
        // ★ 붙여넣기는 `on_event`를 거치지 않는다 — 수거를 여기서도 돌린다(09-04 사용자 실기
        //   "빈 칸에 붙여넣으면 Test가 안 열린다": 키 입력은 on_event 끝의 수거로 즉시 보고됐고 붙여넣기만 빠졌다).
        self.drain_changes(inv);
    }

    /// 우클릭 편집 메뉴 행동(1회성 — 08-13 전수 검사) — 어느 텍스트 입력에서든.
    pub fn take_edit_ctx(&mut self) -> Option<nexa_ctl::controls::EditCtxAction> {
        if let Some(a) = self.search.take_edit_ctx() {
            return Some(a);
        }
        self.rows.iter_mut().find_map(|r| match &mut r.ctl {
            RowCtl::Font { family, size } => family
                .take_edit_ctx()
                .or_else(|| size.editing_input().and_then(|tb| tb.take_edit_ctx())),
            RowCtl::Face(family) => family.take_edit_ctx(),
            RowCtl::Combo(c) => c.editing_input().and_then(|tb| tb.take_edit_ctx()),
            RowCtl::List(l) => l.editing_input().and_then(|tb| tb.take_edit_ctx()),
            _ => None,
        })
    }

    /// 클립보드 텍스트 유무 주입(우클릭 시점 — 붙여넣기 항목 활성 근거).
    pub fn set_clipboard_has_text(&mut self, yes: bool) {
        self.search.set_clipboard_has_text(yes);
        for r in &mut self.rows {
            match &mut r.ctl {
                RowCtl::Font { family, size } => {
                    family.set_clipboard_has_text(yes);
                    if let Some(tb) = size.editing_input() {
                        tb.set_clipboard_has_text(yes);
                    }
                }
                RowCtl::Face(family) => family.set_clipboard_has_text(yes),
                RowCtl::Combo(c) => {
                    if let Some(tb) = c.editing_input() {
                        tb.set_clipboard_has_text(yes);
                    }
                }
                RowCtl::List(l) => {
                    if let Some(tb) = l.editing_input() {
                        tb.set_clipboard_has_text(yes);
                    }
                }
                _ => {}
            }
        }
    }

    /// 검색 텍스트가 코드 경로(잘라내기·붙여넣기)로 바뀌었으면 결과를 재구성한다.
    fn sync_query(&mut self, inv: &mut Invalidations) {
        let q = self.search.text();
        if q != self.query {
            self.query = q;
            self.rebuild(inv);
        }
    }

    /// 배율 지정(고DPI) — 전 컨트롤 전파 + 재구성.
    pub fn set_scale(&mut self, scale: f32, inv: &mut Invalidations) {
        let s = scale.max(0.5);
        if (s - self.scale).abs() > f32::EPSILON {
            self.scale = s;
            self.search.set_scale(s);
            self.rebuild(inv);
        }
    }

    /// 변경된 (키, 새 값) 목록을 꺼낸다(즉시 적용 — 호스트가 반영).
    /// 지정 카테고리로 직행(08-22 — 툴바 서버 표시 클릭 = 서버 설정 바로가기).
    /// 검색은 지우고 스크롤은 맨 위로. 미지 카테고리는 무시.
    pub fn select_category(&mut self, cat: Msg, inv: &mut Invalidations) {
        if let Some(ci) = Self::cats().iter().position(|(c, _)| *c == cat) {
            self.selected = TreeSel::Cat(ci);
            self.query.clear();
            self.search.set_text("");
            self.scroll = 0;
            self.rebuild(inv);
        }
    }
    /// 고급 설정 스위치 상태를 외부에서 맞춘다(열 때 `ui.prefs_advanced` 복원은 `new`가 한다 — 런타임 동기용).
    pub fn set_advanced(&mut self, on: bool, inv: &mut Invalidations) {
        if self.advanced != on {
            self.advanced = on;
            self.adv_switch.set_on(on);
            self.rebuild(inv);
        }
    }
    /// 읽기 전용 정보 행 본문 지정(`SettingKind::Info` · 빈 문자열 = 비움) — 바뀔 때만 다시 그린다.
    pub fn set_info(&mut self, key: &'static str, text: &str, inv: &mut Invalidations) {
        if self.infos.get(key).map(String::as_str) == Some(text) {
            return;
        }
        self.infos.insert(key, text.to_string());
        for row in &mut self.rows {
            if registry()[row.idx].key == key {
                if let RowCtl::Info(t) = &mut row.ctl {
                    *t = text.to_string();
                }
            }
        }
        inv.push(self.bounds);
    }
    /// 현재 선택(시험·호스트 진단용).
    #[must_use]
    pub fn selected(&self) -> TreeSel {
        self.selected
    }

    pub fn take_changes(&mut self) -> Vec<(&'static str, String)> {
        std::mem::take(&mut self.changes)
    }

    /// 검증 실패 경고를 꺼낸다(1회성 — 확정 시 즉시 발생 · 원복은 이미 끝난 뒤).
    pub fn take_warnings(&mut self) -> Vec<Msg> {
        std::mem::take(&mut self.warnings)
    }

    /// 마지막 마우스 위치 — 휠에는 좌표가 없어(09-01) 목록 위인지 판정에 쓴다.
    fn note_mouse(&mut self, ev: &InputEvent) {
        if let InputEvent::MouseDown { x, y, .. }
        | InputEvent::MouseUp { x, y }
        | InputEvent::MouseMove { x, y } = *ev
        {
            self.last_mouse = Point { x, y };
        }
    }

    /// Esc 닫기 요청(1회성).
    pub fn take_back(&mut self) -> bool {
        std::mem::take(&mut self.back)
    }

    fn s(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    /// 카테고리 목록 — **트리 순서**(`CATEGORY_TREE`)로, 레지스트리에 항목이 있는 것만: (카테고리, 그룹 인덱스).
    fn cats() -> Vec<(Msg, usize)> {
        let mut out = Vec::new();
        for (gi, (_, cats)) in CATEGORY_TREE.iter().enumerate() {
            for &c in *cats {
                if registry().iter().any(|e| e.cat == c) {
                    out.push((c, gi));
                }
            }
        }
        out
    }
    /// 이 행이 기본값과 다른가(값 키가 하나라도 다르면 · 값 키 없는 행 = 거짓).
    fn is_modified(&self, idx: usize) -> bool {
        registry()[idx]
            .default_values()
            .iter()
            .any(|(k, d)| self.values.get(k).map(String::as_str) != Some(d.as_str()))
    }
    /// 종속 잠금(DEPENDS) — 부모 값이 조건을 만족하지 않으면 `Some(부모 키, 조건)`.
    fn dep_lock(&self, idx: usize) -> Option<(&'static str, Dep)> {
        let (parent, dep) = depends_of(registry()[idx].key)?;
        let pv = self.values.get(parent).map_or("", String::as_str);
        (!dep.satisfied(pv)).then_some((parent, dep))
    }
    /// 행 아래 한 줄 — 호스트 노트가 우선 · 없으면 종속 잠금 안내.
    fn row_note(&self, idx: usize) -> Option<(String, NoteTone)> {
        let key = registry()[idx].key;
        if let Some((t, tone)) = self.notes.get(key) {
            return Some((t.clone(), *tone));
        }
        let (parent, dep) = self.dep_lock(idx)?;
        let lang = current_lang();
        let plabel = registry()
            .iter()
            .find(|e| e.key == parent)
            .map_or(parent, |e| tr(lang, e.label));
        let text = match dep {
            Dep::On => nclip_core::tf(Msg::PrefsLockedBy, &[plabel]),
            Dep::Eq(v) => {
                // 부모 옵션의 표시 라벨(없으면 값 그대로).
                let vlabel = registry()
                    .iter()
                    .find(|e| e.key == parent)
                    .and_then(|e| match e.kind {
                        SettingKind::Radio(opts) | SettingKind::RadioInput(opts, _) => opts
                            .iter()
                            .find(|(o, _)| *o == v)
                            .map(|(_, m)| tr(lang, *m)),
                        _ => None,
                    })
                    .unwrap_or(v);
                nclip_core::tf(Msg::PrefsLockedByValue, &[plabel, vlabel])
            }
        };
        Some((text, NoteTone::Plain))
    }
    /// 카테고리 매치 수(고급 숨김 반영 — 사이드바 "(N)"은 실제로 보일 수와 같아야 한다).
    fn cat_match_count(&self, cat: Msg, toks: &[String]) -> usize {
        registry()
            .iter()
            .filter(|e| {
                e.cat == cat
                    && !hidden_on_this_os(e.key)
                    && (self.advanced || !is_advanced(e.key))
                    && entry_matches(e, toks)
            })
            .count()
    }
    /// 고급 필터 **전** 후보(검색 또는 선택 범위) — 숨긴 수를 세는 기준.
    fn candidate_indices(&self) -> Vec<usize> {
        let toks = tokens(&self.query);
        let searching = !toks.is_empty();
        let cats = Self::cats();
        let allowed: Vec<Msg> = match self.selected {
            TreeSel::Group(gi) => CATEGORY_TREE
                .get(gi)
                .map_or(Vec::new(), |(_, c)| c.to_vec()),
            TreeSel::Cat(ci) => cats.get(ci).map(|(c, _)| *c).into_iter().collect(),
        };
        registry()
            .iter()
            .enumerate()
            .filter(|(_, e)| !hidden_on_this_os(e.key))
            .filter(|(_, e)| {
                if searching {
                    entry_matches(e, &toks)
                } else {
                    allowed.contains(&e.cat)
                }
            })
            .map(|(i, _)| i)
            .collect()
    }
    /// 가시 항목(registry 인덱스) — 검색 중 = 전 카테고리 매치 · 아니면 선택 범위(그룹/카테고리) ·
    /// 고급 스위치가 꺼져 있으면 [`ADVANCED`] 제외 · 순서 = [`display_order`](그룹 → 카테고리 → 하위 → 접두 → 등재).
    fn visible_indices(&self) -> Vec<usize> {
        let mut hits: Vec<usize> = self
            .candidate_indices()
            .into_iter()
            .filter(|&i| self.advanced || !is_advanced(registry()[i].key))
            .collect();
        hits.sort_by_key(|&i| display_order(i));
        hits
    }
    /// 사이드바·우측 행(컨트롤 포함)을 현재 상태(검색·선택·값)로 다시 만든다.
    fn rebuild(&mut self, inv: &mut Invalidations) {
        let lang = current_lang();
        let toks = tokens(&self.query);
        let searching = !toks.is_empty();
        // ── 사이드바 트리(그룹 → 카테고리 · 10-10 · 검색 중엔 매치만 + "(N)") ──
        let cats = Self::cats();
        self.cat_map.clear();
        let mut roots = Vec::new();
        for (gi, (g, _)) in CATEGORY_TREE.iter().enumerate() {
            let mut children = Vec::new();
            let mut sels = Vec::new();
            let mut gn = 0usize;
            for (ci, (c, cgi)) in cats.iter().enumerate() {
                if *cgi != gi {
                    continue;
                }
                let n = self.cat_match_count(*c, &toks);
                if searching && n == 0 {
                    continue;
                }
                gn += n;
                let label = if searching {
                    format!("{} ({n})", tr(lang, *c))
                } else {
                    tr(lang, *c).to_string()
                };
                children.push(TreeNode::leaf(label));
                sels.push(TreeSel::Cat(ci));
            }
            if children.is_empty() {
                continue;
            }
            let glabel = if searching {
                format!("{} ({gn})", tr(lang, *g))
            } else {
                tr(lang, *g).to_string()
            };
            self.cat_map.push(TreeSel::Group(gi));
            self.cat_map.extend(sels);
            roots.push(TreeNode::branch(glabel, children)); // 기본 펼침
        }
        let mut tree = TreeView::new(TreeModel::new(roots));
        tree.set_scale(self.scale);
        tree.set_focused(true); // 사이드바는 ↑↓ 상시 탐색(트리 자체 포커스 링 없음)
        let sel_row = self
            .cat_map
            .iter()
            .position(|&sel| sel == self.selected)
            .unwrap_or(0);
        tree.set_selected_row(sel_row);
        self.tree = tree;
        let candidates = self.candidate_indices().len();
        // ── 우측 행 + 컨트롤 ──
        self.rows.clear();
        let visible = self.visible_indices();
        self.adv_hidden = candidates.saturating_sub(visible.len());
        // 여러 카테고리가 한 목록에 섞이는 보기(그룹 선택·검색)는 카테고리 경계에 제목을 붙인다.
        let multi_cat = searching || matches!(self.selected, TreeSel::Group(_));
        for idx in visible {
            let e = &registry()[idx];
            let ctl = match e.kind {
                SettingKind::Info => {
                    RowCtl::Info(self.infos.get(e.key).cloned().unwrap_or_default())
                }
                // ★ 숫자 항목 — 라벨이 곧 값이다(번역하지 않는다).
                SettingKind::Number { presets, suffix } => {
                    let items: Vec<ComboItem> =
                        presets.iter().map(|v| ComboItem::new(*v, *v)).collect();
                    let mut c = Combo::new(items, 0);
                    // 후보에 없는 수를 직접 넣는다 — 숫자 필터는 기본값(텍스트 모드 아님).
                    c.set_custom_entry(tr(lang, Msg::CustomInput), suffix);
                    let cur = self.values.get(e.key).map_or("", String::as_str);
                    c.select_value(cur);
                    c.note_value(cur); // 직전 확정값 시드(검증 원복 기준점)
                    c.set_scale(self.scale);
                    RowCtl::Combo(c)
                }
                SettingKind::Radio(opts) | SettingKind::RadioInput(opts, _) => {
                    let items: Vec<ComboItem> = opts
                        .iter()
                        .map(|(v, m)| ComboItem::new(*v, tr(lang, *m)))
                        .collect();
                    let mut c = Combo::new(items, 0);
                    if let SettingKind::RadioInput(_, suffix) = e.kind {
                        c.set_custom_entry(tr(lang, Msg::CustomInput), suffix);
                        // 텍스트 값 행(서버 주소 — 도메인·IP)은 숫자 필터를 푼다(08-22).
                        c.set_custom_text(FREE_TEXT_KEYS.contains(&e.key));
                    }
                    let cur = self.values.get(e.key).map_or("", String::as_str);
                    c.select_value(cur);
                    c.note_value(cur); // 직전 확정값 시드(08-20 — 검증 원복 기준점)
                    c.set_scale(self.scale);
                    RowCtl::Combo(c)
                }
                SettingKind::FontFace { family_key } => {
                    // 기본이 **무엇인지** 보여 준다(사용자 지적 08-10 — "(시스템 기본)"만으로는
                    // 식별 불가). 고정폭 행이므로 고정폭 기본 이름.
                    let ph = if self.default_mono_name.is_empty() {
                        tr(lang, Msg::SystemDefaultFont).to_string()
                    } else {
                        format!(
                            "{} {}",
                            self.default_mono_name,
                            tr(lang, Msg::SystemDefaultFont)
                        )
                    };
                    let mut family = TextBox::new(ph)
                        .with_text(self.values.get(family_key).map_or("", String::as_str));
                    family.set_scale(self.scale);
                    RowCtl::Face(family)
                }
                SettingKind::Text { hint, secret } => {
                    let mut t = TextBox::new(tr(lang, hint))
                        .with_text(self.values.get(e.key).map_or("", String::as_str));
                    t.set_scale(self.scale);
                    t.set_masked(secret); // 기본 은닉(09-03) — 눈 버튼으로 보기.
                    RowCtl::Face(t)
                }
                SettingKind::PositionGrid => {
                    let mut p = PositionPicker::new();
                    p.select_value(self.values.get(e.key).map_or("bl", String::as_str));
                    p.set_scale(self.scale);
                    RowCtl::Pos(p)
                }
                SettingKind::Color { default } => {
                    let mut c =
                        ColorPicker::new(self.values.get(e.key).map_or(default, String::as_str));
                    c.set_scale(self.scale);
                    RowCtl::Color(c)
                }
                SettingKind::Report => RowCtl::Report,
                SettingKind::DeviceList => RowCtl::Devices(build_dev_rows(
                    self.values.get(e.key).map_or("", String::as_str),
                    self.scale,
                    lang,
                )),
                SettingKind::ListEdit => {
                    let mut l = ListEditor::new(
                        self.values.get(e.key).map_or("", String::as_str),
                        tr(lang, Msg::CustomInput),
                    )
                    .with_empty_label(tr(lang, Msg::ListEmpty));
                    l.set_scale(self.scale);
                    RowCtl::List(Box::new(l))
                }
                SettingKind::Toggle => {
                    // mac(iOS) 스타일 스위치(08-11 사용자 요청) — 라벨은 행 왼쪽 제목이
                    // 이미 있으므로 토글만([`LabelSide::None`]).
                    let mut c =
                        Switch::new("", self.values.get(e.key).map(String::as_str) == Some("on"))
                            .with_label_side(LabelSide::None);
                    c.set_scale(self.scale);
                    RowCtl::Check(c)
                }
                SettingKind::Action { verb } => {
                    let mut b = Button::new(tr(lang, verb));
                    b.set_scale(self.scale);
                    RowCtl::Act(b)
                }
                SettingKind::Hotkey { .. } => {
                    let mut b = Button::new(hotkey_label(
                        self.values.get(e.key).map_or("", String::as_str),
                        lang,
                    ));
                    b.set_scale(self.scale);
                    RowCtl::Act(b)
                }
                SettingKind::FontSection {
                    family_key,
                    size_key,
                } => {
                    let ph = if self.default_base_name.is_empty() {
                        tr(lang, Msg::SystemDefaultFont).to_string()
                    } else {
                        format!(
                            "{} {}",
                            self.default_base_name,
                            tr(lang, Msg::SystemDefaultFont)
                        )
                    };
                    let mut family = TextBox::new(ph)
                        .with_text(self.values.get(family_key).map_or("", String::as_str));
                    family.set_scale(self.scale);
                    let items: Vec<ComboItem> = FONT_SIZE_OPTS
                        .iter()
                        .map(|(v, m)| ComboItem::new(*v, tr(lang, *m)))
                        .collect();
                    let mut size = Combo::new(items, 0);
                    // 숫자 직접 입력(08-18 사용자 요청) — 프리셋도 절대 px 값이라
                    // 같은 축이다(해석·클램프는 소비 측 fonts_from_settings).
                    size.set_custom_entry(tr(lang, Msg::CustomInput), "px");
                    size.select_value(
                        self.values
                            .get(size_key)
                            .map_or(FONT_SIZE_DEFAULT, String::as_str),
                    );
                    size.set_scale(self.scale);
                    // Box = clippy large_enum_variant(10-10 nexa-ctl TextBox가 커짐) — 행은 수십 개라 비용 없음.
                    RowCtl::Font {
                        family: Box::new(family),
                        size,
                    }
                }
            };
            // 제목 규칙: 카테고리가 바뀌면(여러 카테고리 보기) 카테고리 제목 · 같은 카테고리 안에서 하위 섹션이
            // 바뀌면 하위 제목 · 직속 구간은 없음(상위 제목은 고정 밴드 몫).
            let group = (e.cat, e.sub);
            let head = match self.rows.last().map(|r| r.group) {
                Some((pc, ps)) if pc == e.cat => e.sub.filter(|_| ps != e.sub),
                _ if multi_cat => Some(e.cat),
                _ => e.sub,
            };
            // [초기화] — 값 키가 있는 행만(행위·정보·보고 행은 없다). 보이기는 layout이 `is_modified`로 정한다.
            let resettable = !e.default_values().is_empty()
                && !matches!(e.kind, SettingKind::Report | SettingKind::DeviceList);
            let reset = resettable.then(|| {
                let mut b = Button::new(tr(lang, Msg::BtnReset));
                b.set_scale(self.scale);
                b
            });
            self.rows.push(RowUi {
                idx,
                rect: Rect::default(),
                ctl,
                group,
                head,
                head_h: 0,
                desc_lines: 1,
                desc_avail: 0,
                reset,
                key_rect: std::cell::Cell::new(Rect::default()),
            });
        }
        self.layout(inv);
    }

    /// 값을 외부에서 갱신한다(예: 기간 만료로 승인 방식이 되돌아갔을 때) —
    /// 화면과 실제가 어긋나지 않게 콤보 표시까지 맞춘다.
    pub fn set_value(&mut self, key: &'static str, value: &str, inv: &mut Invalidations) {
        self.values.insert(key, value.to_string());
        for row in &mut self.rows {
            if registry()[row.idx].key != key {
                continue;
            }
            match &mut row.ctl {
                RowCtl::Combo(c) => c.select_value(value),
                RowCtl::List(l) => l.set_value(value),
                // ★ 기기 목록(09-04) — 줄들로 행·버튼을 다시 만든다(레이아웃은 호출자가 강제).
                RowCtl::Devices(rows) => *rows = build_dev_rows(value, self.scale, current_lang()),
                // 토글도 역반영(08-15 — 쌍방 동기화: 다른 경로가 켠/끈 것을 표시).
                RowCtl::Check(c) => c.set_on(value == "on"),
                // ★ 텍스트 입력도(09-03 — 패스프레이즈 추천/생성을 재구성 없이 반영).
                // ★ 단축키 행(09-04): 버튼 글자 = 조합(빈 값 = 없음).
                RowCtl::Act(b)
                    if matches!(registry()[row.idx].kind, SettingKind::Hotkey { .. }) =>
                {
                    b.set_label(hotkey_label(value, current_lang()));
                }
                RowCtl::Face(f) => f.set_text(value),
                _ => {}
            }
        }
        inv.push(self.bounds);
    }

    /// 비활성 키 지정 — 조건부로만 쓰이는 설정을 흐리게 잠근다(예: 기간은 "기간 자동"일 때만).
    pub fn set_disabled(&mut self, keys: &[&'static str], inv: &mut Invalidations) {
        let next: std::collections::HashSet<&'static str> = keys.iter().copied().collect();
        if next != self.disabled {
            self.disabled = next;
            inv.push(self.bounds);
        }
    }

    /// ★ 행위 버튼 톤(09-04 사용자 — 암호 재생성 버튼과 같은 2단계 무장 표시): 무장 중 = Danger.
    pub fn set_action_tone(
        &mut self,
        key: &'static str,
        tone: nexa_ctl::controls::ButtonTone,
        inv: &mut Invalidations,
    ) {
        for r in &mut self.rows {
            if registry()[r.idx].key == key {
                if let RowCtl::Act(b) = &mut r.ctl {
                    b.set_tone(tone);
                    inv.push(r.rect);
                }
            }
        }
    }

    /// 설정 행 아래 한 줄 정보 지정 — **자리가 고정**된다(빈 문자열 = 제거).
    /// 값이 바뀔 때만 재배치·무효화하므로 1초 갱신에도 낭비가 없다.
    pub fn set_row_note(&mut self, key: &'static str, text: &str, inv: &mut Invalidations) {
        self.set_row_note_toned(key, text, NoteTone::Plain, inv);
    }

    /// 톤 있는 행 노트(08-22) — Ok/Warn은 옅은 배경으로 눈에 띈다(검증 상태 표시).
    pub fn set_row_note_toned(
        &mut self,
        key: &'static str,
        text: &str,
        tone: NoteTone,
        inv: &mut Invalidations,
    ) {
        let had = self.notes.contains_key(key);
        if self.notes.get(key).map(|(t, tn)| (t.as_str(), *tn)) == Some((text, tone))
            || (text.is_empty() && !had)
        {
            return;
        }
        if text.is_empty() {
            self.notes.remove(key);
        } else {
            self.notes.insert(key, (text.to_string(), tone));
        }
        if had != self.notes.contains_key(key) {
            self.layout(inv); // 줄이 생기거나 사라지면 행 높이가 달라진다
        }
        inv.push(self.bounds);
    }

    /// 이 행에 붙은 정보 줄 높이(없으면 0) — 노트는 카드 안 컨트롤 줄 아래(간격 6 + 노트).
    fn note_h(&self, idx: usize) -> i32 {
        if self.row_note(idx).is_some() {
            self.s(NOTE_H + 6)
        } else {
            0
        }
    }
    /// 이 행이 잠겼는가 — 호스트 런타임 잠금 ∪ 종속(DEPENDS) 불충족.
    fn is_locked(&self, idx: usize) -> bool {
        self.disabled.contains(registry()[idx].key) || self.dep_lock(idx).is_some()
    }
    /// 하단 줄 높이(물리 px).
    fn bottom_h(&self) -> i32 {
        self.s(BOTTOM_H)
    }
    /// 하단 줄 영역.
    fn bottom_rect(&self) -> Rect {
        let b = self.bounds;
        let h = self.bottom_h();
        Rect::new(b.x, b.bottom() - h, b.w, h)
    }
    /// 상단 고정 밴드(상위 + 하위 제목 + 고급 숨김 배너) 높이 — 하위가 없어도 **줄어들지 않는다**.
    /// 그룹 경계를 넘을 때 아래 내용이 위아래로 튀면 읽던 자리를 잃는다.
    fn crumb_h(&self) -> i32 {
        self.s(CRUMB_CAT_H)
            + self.s(CRUMB_SUB_H)
            + if self.adv_hidden > 0 {
                self.s(BANNER_H)
            } else {
                0
            }
    }
    /// 우측 패널 뷰포트(사이드바 제외 · **고정 밴드 아래**부터 · 하단 줄 위까지).
    fn right_viewport(&self) -> Rect {
        let sw = self.s(self.sidebar_w);
        let b = self.bounds;
        let top = b.y + self.crumb_h();
        let bottom = b.bottom() - self.bottom_h();
        Rect::new(b.x + sw, top, (b.w - sw).max(0), (bottom - top).max(0))
    }

    /// 스크롤 위치 기준으로 지금 보이는 그룹 `(상위, 하위)` — 고정 밴드가 이걸 그린다.
    /// 뷰포트 맨 위에 걸친 행의 그룹을 쓴다(그 행이 곧 사용자가 지금 읽는 것).
    fn current_group(&self) -> Option<(Msg, Option<Msg>)> {
        let vp = self.right_viewport();
        self.rows
            .iter()
            .find(|r| r.rect.bottom() > vp.y)
            .or_else(|| self.rows.last())
            .map(|r| r.group)
    }

    /// 스크롤바 자동숨김 + ★ **hover 페이드** 틱 — 표시가 바뀌면 `true`(재그리기).
    ///
    /// ⚠️ **`||` 단축 평가를 쓰면 안 된다** — 앞이 참이면 뒤가 시간을 못 흘려서
    /// 그 컨트롤의 페이드가 멈춘다. 전부 `|`로 묶는다.
    pub fn tick(&mut self, now_ms: u64) -> bool {
        let mut dirty = self.bars.tick(now_ms) | self.tree.tick(now_ms);
        // ★ 생성 무장 타이머 — 무장 중엔 계속 깨워 만료를 제때 잡는다(≤ 2초).
        if let Some(t) = self.pw_arm {
            if t.elapsed() > PW_ARM_WINDOW {
                self.pw_arm = None;
            }
            dirty = true;
        }
        self.split_fade.set(self.split_hover || self.split_drag);
        dirty |= self.split_fade.tick(now_ms);
        // 행 컨트롤 — 콤보(닫힌 박스)와 버튼이 각자 자기 페이드를 옮긴다.
        for row in &mut self.rows {
            dirty |= match &mut row.ctl {
                RowCtl::Combo(c) => c.tick_hover(now_ms),
                RowCtl::Act(b) => b.tick(now_ms),
                RowCtl::Devices(rows) => rows.iter_mut().fold(false, |d, r| d | r.tick(now_ms)),
                RowCtl::List(l) => l.tick(now_ms),
                RowCtl::Font { size, .. } => size.tick_hover(now_ms),
                _ => false,
            };
            if let Some(b) = &mut row.reset {
                dirty |= b.tick(now_ms);
            }
        }
        dirty |= self.btn_file.tick(now_ms) | self.btn_close.tick(now_ms);
        dirty
    }

    /// 스플리터 무효화 사각형(글로우가 번지는 띠).
    fn split_rect(&self) -> Rect {
        let split_x = self.bounds.x + self.s(self.sidebar_w);
        Rect::new(split_x - self.s(4), self.bounds.y, self.s(9), self.bounds.h)
    }

    /// 이 좌표에서 좌우 리사이즈 커서를 보여야 하는가 — 스플리터 hover/드래그
    /// (호스트가 OS 커서로 번역 · 사용자 요청 08-09: 조절 가능함을 직관적으로).
    #[must_use]
    pub fn wants_col_resize_cursor(&self, x: i32, y: i32) -> bool {
        if self.split_drag {
            return true;
        }
        let split_x = self.bounds.x + self.s(self.sidebar_w);
        (x - split_x).abs() <= self.s(4) && y >= self.bounds.y && y < self.bounds.bottom()
    }

    /// 현 bounds에 맞춰 자식 컨트롤 배치 — ★ 카드 레이아웃(10-10 · beep P2 · nexa-sql 모양).
    fn layout(&mut self, inv: &mut Invalidations) {
        let sw = self.s(self.sidebar_w);
        let b = self.bounds;
        self.search.set_bounds(
            Rect::new(
                b.x + self.s(4),
                b.y + self.s(4),
                sw - self.s(8),
                self.s(SEARCH_H),
            ),
            inv,
        );
        let tree_top = b.y + self.s(SEARCH_H) + self.s(8);
        let bottom_top = b.bottom() - self.bottom_h();
        self.tree.set_bounds(
            Rect::new(b.x, tree_top, sw, (bottom_top - tree_top).max(0)),
            inv,
        );
        // ── 하단 줄(10-10): [고급 스위치] ………… [설정 파일 열기…][닫기] ──
        {
            let ctl_h = self.s(CTL_H);
            let pad = self.s(PAD);
            let cy = bottom_top + (self.bottom_h() - ctl_h) / 2;
            self.adv_switch.set_scale(self.scale);
            self.adv_switch
                .set_bounds(Rect::new(b.x + pad, cy, self.s(190), ctl_h), inv);
            let close_w = self.s(90);
            let file_w = self.s(150);
            self.btn_close.set_scale(self.scale);
            self.btn_close.set_bounds(
                Rect::new(b.right() - pad - close_w, cy, close_w, ctl_h),
                inv,
            );
            self.btn_file.set_scale(self.scale);
            self.btn_file.set_bounds(
                Rect::new(
                    b.right() - pad - close_w - self.s(8) - file_w,
                    cy,
                    file_w,
                    ctl_h,
                ),
                inv,
            );
        }
        let rx = b.x + sw; // 우측 패널 시작
        let rw = (b.w - sw).max(0);
        // ── 카드 레이아웃(10-10 · nexa-sql 모양) ──
        //   ┌ 제목 ……………………………… 키 이름 ⧉ ┐
        //   │ 설명(워드랩 1~3줄)                      │
        //   │ [컨트롤] [초기화]           기본값: … │
        //   └ (노트 — 호스트/종속 잠금)              ┘
        //   목록·보고·기기 목록은 컨트롤 줄이 카드 전폭 여러 줄.
        let (ctl_h, pad, cpad, gap) = (
            self.s(CTL_H),
            self.s(PAD),
            self.s(CARD_PAD),
            self.s(CARD_GAP),
        );
        let (combo_w, check_w) = (self.s(COMBO_W), self.s(nexa_ctl::controls::ctl_size(20)));
        let (family_w, size_w, gap10, text_w) =
            (self.s(FAMILY_W), self.s(SIZE_W), self.s(10), self.s(TEXT_W));
        let dev_g = self.s(6);
        let lang = current_lang();
        let scale = self.scale;
        let desc_line_h = self.s(DESC_LINE_H);
        let title_h = self.s(TITLE_H);
        let head_h = self.s(SUB_HEAD_H);
        let reset_w = self.s(RESET_W);
        let card_x = rx + pad;
        let card_w = (rw - pad * 2).max(self.s(120));
        let inner_w = (card_w - cpad * 2).max(self.s(60));
        let desc_avail = inner_w;
        let desc_gap = self.s(2);
        let ctl_gap = self.s(8);
        // 행별 치수를 **한 번** 계산해 총높이와 배치가 같은 값을 쓴다(08-15 상한 불일치 재발 방지).
        struct Metric {
            desc_lines: i32,
            ctl_block: i32,
            h: i32,
        }
        let metrics: Vec<Metric> = self
            .rows
            .iter()
            .map(|row| {
                let e = &registry()[row.idx];
                // 설명 줄 수 추정(ASCII 7·그 외 14 논리px — 실측은 페인트가 하고 여기는 **예약**).
                let est_logical: i32 = tr(lang, e.desc)
                    .chars()
                    .map(|c| if c.is_ascii() { 7 } else { 14 })
                    .sum();
                #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                let est_px = (est_logical as f32 * scale).round() as i32;
                let desc_lines = ((est_px + desc_avail - 1) / desc_avail).clamp(1, 3);
                // 컨트롤 블록 높이 — 대부분 한 줄 · 목록/보고/기기 목록/위치 그리드는 제 높이.
                let ctl_block = match &row.ctl {
                    RowCtl::List(l) => l.preferred_height(),
                    RowCtl::Report => report_lines(self.values.get(e.key)).max(1) * desc_line_h,
                    RowCtl::Devices(rows) => {
                        dev_rows_h(rows, card_w, cpad, dev_g, ctl_h, desc_line_h, scale).max(ctl_h)
                    }
                    RowCtl::Pos(p) => p.preferred_size().1.max(ctl_h),
                    _ => ctl_h,
                };
                let h = cpad
                    + title_h
                    + desc_gap
                    + desc_lines * desc_line_h
                    + ctl_gap
                    + ctl_block
                    + self.note_h(row.idx)
                    + cpad;
                Metric {
                    desc_lines,
                    ctl_block,
                    h,
                }
            })
            .collect();
        self.content_h = self
            .rows
            .iter()
            .zip(&metrics)
            .map(|(row, m)| m.h + gap + if row.head.is_some() { head_h } else { 0 })
            .sum();
        let vp_h = self.right_viewport().h;
        self.scroll = self.scroll.clamp(0, (self.content_h - vp_h).max(0));
        // [초기화] 표시 여부 = 기본값과 다름 ∧ 잠기지 않음.
        let show_reset: Vec<bool> = self
            .rows
            .iter()
            .map(|r| self.is_modified(r.idx) && !self.is_locked(r.idx))
            .collect();
        let vp_bottom = self.bounds.bottom() - self.bottom_h();
        let min_color_w = self.s(80);
        // 내용은 **밴드 아래**에서 시작한다(밴드가 첫 행을 가리면 못 만진다).
        let mut top = b.y + self.crumb_h() - self.scroll;
        for (ri, row) in self.rows.iter_mut().enumerate() {
            let e = &registry()[row.idx];
            let m = &metrics[ri];
            row.desc_avail = desc_avail;
            row.desc_lines = m.desc_lines;
            row.head_h = if row.head.is_some() { head_h } else { 0 };
            top += row.head_h;
            row.rect = Rect::new(card_x, top, card_w, m.h);
            // 컨트롤 줄 = 좌하단(제목·설명 아래).
            let cy = top + cpad + title_h + desc_gap + m.desc_lines * desc_line_h + ctl_gap;
            let cx = card_x + cpad;
            let secret = matches!(e.kind, SettingKind::Text { secret: true, .. });
            let text_row = matches!(e.kind, SettingKind::Text { .. });
            let ctl_right = match &mut row.ctl {
                RowCtl::Combo(c) => {
                    c.set_bounds(Rect::new(cx, cy, combo_w, ctl_h), inv);
                    c.set_viewport_bottom(vp_bottom); // 아래 끝 행의 팝업이 잘리지 않게(08-20)
                    cx + combo_w
                }
                RowCtl::Check(c) => {
                    c.set_bounds(Rect::new(cx, cy, check_w, ctl_h), inv);
                    cx + check_w
                }
                RowCtl::Font { family, size } => {
                    family.set_bounds(Rect::new(cx, cy, family_w, ctl_h), inv);
                    size.set_bounds(Rect::new(cx + family_w + gap10, cy, size_w, ctl_h), inv);
                    size.set_viewport_bottom(vp_bottom);
                    cx + family_w + gap10 + size_w
                }
                RowCtl::Face(f) if text_row => {
                    f.set_bounds(Rect::new(cx, cy, text_w, ctl_h), inv);
                    // 비밀 행은 상자 오른쪽에 [생성][눈] 두 칸(ctl_h 정사각 · 간격 ctl_h/8).
                    cx + text_w + if secret { ctl_h / 8 * 2 + ctl_h * 2 } else { 0 }
                }
                RowCtl::Face(f) => {
                    f.set_bounds(Rect::new(cx, cy, family_w, ctl_h), inv);
                    cx + family_w
                }
                RowCtl::Pos(p) => {
                    p.set_scale(scale);
                    let (pw, ph) = p.preferred_size();
                    p.set_bounds(Rect::new(cx, cy, pw, ph), inv);
                    cx + pw
                }
                RowCtl::Color(c) => {
                    c.set_scale(scale);
                    let cw = c
                        .preferred_width()
                        .min((inner_w - reset_w - gap10).max(min_color_w));
                    c.set_bounds(Rect::new(cx, cy, cw, ctl_h), inv);
                    cx + cw
                }
                RowCtl::Act(b) => {
                    b.set_scale(scale);
                    b.set_bounds(Rect::new(cx, cy, combo_w, ctl_h), inv);
                    cx + combo_w
                }
                RowCtl::List(l) => {
                    l.set_scale(scale);
                    l.set_bounds(Rect::new(cx, cy, inner_w, m.ctl_block), inv);
                    cx + inner_w
                }
                RowCtl::Report => cx + inner_w,
                RowCtl::Devices(rows) => {
                    // 행마다: 텍스트(왼쪽) + [승인|해제][삭제](오른쪽 정렬) — 카드 안쪽 폭 기준.
                    let g = dev_g;
                    let bh = dev_btn_h(ctl_h);
                    let mut y = cy;
                    for r in rows.iter_mut() {
                        r.y = y;
                        r.text_w = dev_text_w(r, card_w, cpad, g);
                        r.lines = est_lines(&r.text, r.text_w, scale);
                        let row_h = (r.lines * desc_line_h).max(bh);
                        let (bw, dw) = (r.bw, r.dw);
                        let by = y + (desc_line_h.max(bh) - bh) / 2;
                        if let Some(b) = r.del.as_mut() {
                            b.set_scale(scale);
                            b.set_bounds(Rect::new(card_x + card_w - cpad - dw, by, dw, bh), inv);
                        }
                        if let Some(b) = r.approve.as_mut() {
                            b.set_scale(scale);
                            b.set_bounds(
                                Rect::new(card_x + card_w - cpad - dw - g - bw, by, bw, bh),
                                inv,
                            );
                        }
                        y += row_h + g;
                    }
                    cx + inner_w
                }
                RowCtl::Info(_) => cx + text_w,
            };
            // [초기화] — 컨트롤 오른쪽(값이 기본값과 다를 때만 · 잠기면 숨김).
            if let Some(btn) = &mut row.reset {
                btn.set_scale(scale);
                let rect = if show_reset[ri] {
                    Rect::new(ctl_right + gap10, cy, reset_w, ctl_h)
                } else {
                    Rect::default()
                };
                btn.set_bounds(rect, inv);
            }
            top += m.h + gap;
        }
        inv.push(self.bounds);
    }

    /// 자식 컨트롤 변경분을 회수해 values/changes에 반영.
    fn drain_changes(&mut self, inv: &mut Invalidations) {
        let mut got = Vec::new();
        let mut warn: Vec<Msg> = Vec::new();
        for row in &mut self.rows {
            let e = &registry()[row.idx];
            match &mut row.ctl {
                RowCtl::Combo(c) => {
                    if let Some(v) = c.take_changed() {
                        // 검증(08-20) — 실패 = 경고 + **직전 확정값 원복**(ControlBase
                        // last_value 상속 · rebuild가 현재값을 시드, 성공 확정마다 갱신).
                        match validate(e.key, &v) {
                            Ok(()) => {
                                c.note_value(v.clone());
                                got.push((e.key, v));
                            }
                            Err(msg) => {
                                let prev = c
                                    .last_value()
                                    .map(str::to_owned)
                                    .or_else(|| self.values.get(e.key).cloned())
                                    .unwrap_or_default();
                                c.select_value(&prev);
                                warn.push(msg);
                            }
                        }
                    }
                }
                RowCtl::Pos(g) => {
                    if let Some(v) = g.take_changed() {
                        got.push((e.key, v));
                    }
                }
                RowCtl::List(l) => {
                    if let Some(v) = l.take_changed() {
                        got.push((e.key, v));
                    }
                }
                RowCtl::Report => {}
                RowCtl::Devices(rows) => {
                    for r in rows.iter_mut() {
                        if r.approve.as_mut().is_some_and(Button::take_clicked) {
                            let verb = if r.approved { "revoke" } else { "approve" };
                            got.push((e.key, format!("{verb}:{}", r.hex)));
                        }
                        if r.del.as_mut().is_some_and(Button::take_clicked) {
                            got.push((e.key, format!("delete:{}", r.hex)));
                        }
                    }
                }
                RowCtl::Face(family) => {
                    // ★ 글꼴 행은 글자마다 폰트를 찾으면 낭비라 **Enter 확정**만 보고한다(08-09).
                    //   ★ 자유 문자열 행(`Text` — 핸들·암호·서버·포트·기기 이름)은 **글자마다** 보고한다
                    //   (09-04 사용자 실기 "비우고 Enter를 눌러야 효과" — 수정 즉시 잠금·해제가 돌아야 한다).
                    let immediate = matches!(e.kind, SettingKind::Text { .. });
                    if let Some(v) = family.take_committed() {
                        got.push((e.key, v));
                        let _ = family.take_changed();
                    } else if let Some(v) = family.take_changed() {
                        if immediate {
                            got.push((e.key, v));
                        }
                    }
                }
                RowCtl::Color(c) => {
                    if let Some(v) = c.take_changed() {
                        got.push((e.key, v));
                    }
                }
                RowCtl::Check(c) => {
                    if let Some(on) = c.take_toggled() {
                        got.push((e.key, if on { "on" } else { "off" }.to_string()));
                    }
                }
                RowCtl::Act(b) => {
                    // 행위 항목 — 값이 아니라 트리거. 호스트가 key로 분기한다.
                    if b.take_clicked() {
                        got.push((e.key, "run".to_string()));
                    }
                }
                RowCtl::Font { family, size } => {
                    if let SettingKind::FontSection {
                        family_key,
                        size_key,
                    } = e.kind
                    {
                        // 글꼴명은 **확정 시점만** 보고(08-18 사용자 요청 — 글자마다
                        // 리로드 낭비 · Face 행과 같은 규약). 포커스 아웃 확정은
                        // 위젯 on_event의 blur 수확이 같은 경로로 밀어 넣는다.
                        if let Some(v) = family.take_committed() {
                            got.push((family_key, v));
                        }
                        let _ = family.take_changed(); // 중간 변경은 버린다
                        if let Some(v) = size.take_changed() {
                            got.push((size_key, v));
                        }
                    }
                }
                RowCtl::Info(_) => {}
            }
        }
        // [초기화](10-10) — 그 행의 값 키 전부를 기본값으로(FontSection = family+size).
        let mut reset_any = false;
        for row in &mut self.rows {
            if let Some(b) = &mut row.reset {
                if b.take_clicked() {
                    for (k, d) in registry()[row.idx].default_values() {
                        got.push((k, d));
                    }
                    reset_any = true;
                }
            }
        }
        if !got.is_empty() {
            for (k, v) in &got {
                self.values.insert(k, v.clone());
            }
            self.changes.extend(got);
            inv.push(self.bounds);
            if reset_any {
                // 컨트롤 표시를 값에 맞춘다(종류마다 역반영 API가 달라 재구성이 가장 확실하다).
                self.rebuild(inv);
            } else {
                // 기본값 여부([초기화] 노출)·종속 잠금(부모 값)이 바뀌었을 수 있다 — 재배치.
                self.layout(inv);
            }
        }
        if !warn.is_empty() {
            self.warnings.extend(warn);
            inv.push(self.bounds); // 원복된 표시를 즉시 갱신
        }
    }

    /// 열린 콤보(모달 캡처 대상)를 찾는다.
    fn open_combo_mut(&mut self) -> Option<&mut Combo> {
        self.rows.iter_mut().find_map(|r| match &mut r.ctl {
            RowCtl::Combo(c) if c.is_open() => Some(c),
            RowCtl::Font { size, .. } if size.is_open() => Some(size),
            _ => None,
        })
    }

    /// 시스템 기본 폰트의 표시 이름 지정 — "(시스템 기본)"이 무엇인지 placeholder에
    /// 보여 준다(사용자 지적 08-10). 호스트가 plat에서 조회해 넣는다(ui는 OS를 모른다).
    pub fn set_default_font_names(&mut self, base: &str, mono: &str, inv: &mut Invalidations) {
        if self.default_base_name != base || self.default_mono_name != mono {
            self.default_base_name = base.to_string();
            self.default_mono_name = mono.to_string();
            self.rebuild(inv);
        }
    }

    /// 카드 우상단 키 이름 + ⧉(10-10) — 고급 키는 accent · 자리는 `key_rect`에 남겨 클릭 = 키 복사.
    fn paint_key(
        &self,
        ctx: &mut dyn DrawCtx,
        theme: &Theme,
        row: &RowUi,
        right: i32,
        y: i32,
        clip: Rect,
    ) {
        let key = registry()[row.idx].key;
        let color = if is_advanced(key) {
            theme.accent
        } else {
            theme.text_dim
        };
        ctx.select_font(FontSlot::Base, false);
        let bh = ctx.text_height();
        ctx.select_font(FontSlot::Status, false);
        let th = ctx.text_height();
        let glyph = "⧉";
        let kw = ctx.text_width(key);
        let gw = ctx.text_width(glyph);
        let w = kw + self.s(4) + gw;
        let kx = right - w;
        let ky = y + (bh - th) / 2;
        ctx.text(kx, ky, clip, key, color);
        ctx.text(kx + kw + self.s(4), ky, clip, glyph, color);
        row.key_rect
            .set(Rect::new(kx, y, w, bh).intersection(&clip));
    }

    fn any_family_focused(&self) -> bool {
        // ★ Face(얼굴만 지정 — 고정폭)도 글꼴명 입력이다 — 여기서 빠지면 그 입력이
        // "기본 타이핑 = 검색" 폴백으로 새어 검색창에 글자가 들어간다(사용자 지적 08-10).
        // Color의 hex 입력도 같은 부류(같은 사고를 반복하지 않는다).
        self.rows.iter().any(|r| match &r.ctl {
            RowCtl::Font { family, .. } => family.is_focused(),
            RowCtl::Face(f) => f.is_focused(),
            RowCtl::Color(c) => c.hex_focused(),
            _ => false,
        })
    }

    /// ★ 목록이 키를 받을 상태(포커스·편집)인가 — 이 때 ↑↓가 사이드바로 샐면
    /// 메뉴가 움직인다(09-01 사용자 실기).
    fn any_list_wants_keys(&self) -> bool {
        self.rows
            .iter()
            .any(|r| matches!(&r.ctl, RowCtl::List(l) if l.wants_keys()))
    }

    /// 목록 행 인라인 편집 중인가 — 문자 입력이 검색으로 샐면 안 된다.
    fn any_list_editing(&self) -> bool {
        self.rows
            .iter()
            .any(|r| matches!(&r.ctl, RowCtl::List(l) if l.is_editing()))
    }

    /// 키 입력을 활성 목록으로 보내고 변경을 수확한다.
    fn route_key_to_list(&mut self, ev: &InputEvent, inv: &mut Invalidations) {
        for row in &mut self.rows {
            if let RowCtl::List(l) = &mut row.ctl {
                if l.wants_keys() {
                    l.on_event(ev, inv);
                }
            }
        }
        self.drain_changes(inv);
        inv.push(self.bounds);
    }
}

impl Widget for SettingsWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect, inv: &mut Invalidations) {
        self.bounds = bounds;
        self.layout(inv);
    }

    fn on_event(&mut self, ev: &InputEvent, inv: &mut Invalidations) {
        // ── 글꼴명 blur 확정(08-18 사용자 요청 "Enter 또는 포커스 아웃에 반영") —
        //    포커스된 글꼴명 밖을 클릭하면 미확정 텍스트를 그 자리에서 확정 보고한다
        //    (Enter의 take_committed와 같은 경로 · Esc는 취소라 여기 안 온다).
        if let &InputEvent::MouseDown { x, y, .. } = ev {
            let mut got: Vec<(&'static str, String)> = Vec::new();
            for r in &mut self.rows {
                let e = &registry()[r.idx];
                let (family, key) = match (&mut r.ctl, e.kind) {
                    (RowCtl::Font { family, .. }, SettingKind::FontSection { family_key, .. }) => {
                        (family.as_mut(), family_key)
                    }
                    (RowCtl::Face(family), _) => (family, e.key),
                    _ => continue,
                };
                if family.is_focused() && !family.bounds().contains(Point { x, y }) {
                    let v = family.text().trim().to_string();
                    if self.values.get(key).map(String::as_str) != Some(v.as_str()) {
                        got.push((key, v));
                    }
                }
            }
            if !got.is_empty() {
                for (k, v) in &got {
                    self.values.insert(k, v.clone());
                }
                self.changes.extend(got);
                inv.push(self.bounds);
            }
        }
        // ── 모달 캡처: 열린 콤보가 있으면 그 콤보만 이벤트를 받는다(전파 차단) ──
        if let Some(c) = self.open_combo_mut() {
            c.on_event(ev, inv);
            self.drain_changes(inv);
            inv.push(self.bounds); // 드롭다운 영역 재그리기
            return;
        }
        // ── 인라인 편집(직접 입력) 모달 캡처 — 편집 중 콤보가 모든 입력을 받는다 ──
        // FontSection의 크기 콤보도 포함(08-18 실기 — 빠져 있어 커스텀 px 입력이
        // 검색란으로 샜다: 캐럿은 콤보에, 글자는 검색에 가는 어긋남).
        if let Some(c) = self.rows.iter_mut().find_map(|r| match &mut r.ctl {
            RowCtl::Combo(c) if c.is_editing() => Some(c),
            RowCtl::Font { size, .. } if size.is_editing() => Some(size),
            _ => None,
        }) {
            c.on_event(ev, inv);
            self.drain_changes(inv);
            inv.push(self.bounds);
            return;
        }
        // ── 포커스된 글꼴명 텍스트박스 — **편집 이벤트 일반 라우팅**(08-18 사용자
        //    지적: 드래그 선택·우클릭 메뉴·전체 선택이 공통 기능인데 컨테이너의
        //    키 화이트리스트가 끊었다 — 기능은 TextBox에 이미 있다).
        //    우클릭 편집 메뉴가 열려 있으면 그 박스가 **모달로 전부** 받고,
        //    아니면 Move/Up(드래그 추적)·안쪽 우클릭·SelectAll을 흘린다.
        //    Char·Enter·이동 키는 기존 arm 그대로.
        {
            let mut handled = false;
            if let Some(f) = self.rows.iter_mut().find_map(|r| match &mut r.ctl {
                RowCtl::Font { family, .. } if family.popup_open() || family.is_focused() => {
                    Some(family.as_mut())
                }
                RowCtl::Face(family) if family.popup_open() || family.is_focused() => Some(family),
                _ => None,
            }) {
                if f.popup_open() {
                    f.on_event(ev, inv);
                    handled = true;
                } else {
                    match *ev {
                        InputEvent::MouseMove { .. } | InputEvent::MouseUp { .. } => {
                            f.on_event(ev, inv); // 드래그 선택 추적(비캡처 — 아래로도 흐른다)
                        }
                        InputEvent::RightDown { x, y } if f.bounds().contains(Point { x, y }) => {
                            f.on_event(ev, inv); // 편집 메뉴 열기
                            handled = true;
                        }
                        InputEvent::SelectAll => {
                            f.on_event(ev, inv);
                            handled = true;
                        }
                        _ => {}
                    }
                }
            }
            if handled {
                self.drain_changes(inv);
                inv.push(self.bounds);
                return;
            }
        }
        // ── 하단 줄(10-10): 고급 스위치 · 설정 파일 열기 · 닫기 ──
        {
            let in_bar = match *ev {
                InputEvent::MouseDown { x, y, .. }
                | InputEvent::MouseUp { x, y }
                | InputEvent::MouseMove { x, y } => self.bottom_rect().contains(Point { x, y }),
                _ => false,
            };
            // MouseUp은 누른 컨트롤이 떼는 자리를 봐야 하므로 항상 흘린다(안쪽에서 눌러 밖에서 떼면 취소).
            if in_bar
                || matches!(
                    *ev,
                    InputEvent::MouseUp { .. } | InputEvent::MouseMove { .. }
                )
            {
                self.adv_switch.on_event(ev, inv);
                self.btn_file.on_event(ev, inv);
                self.btn_close.on_event(ev, inv);
                if let Some(on) = self.adv_switch.take_toggled() {
                    self.advanced = on;
                    self.changes.push((
                        "ui.prefs_advanced",
                        if on { "on" } else { "off" }.to_string(),
                    ));
                    self.rebuild(inv);
                    inv.push(self.bounds);
                    return;
                }
                if self.btn_close.take_clicked() {
                    self.back = true;
                    inv.push(self.bounds);
                    return;
                }
                if self.btn_file.take_clicked() {
                    self.changes.push(("settings.open_file", "run".to_string()));
                    inv.push(self.bounds);
                    return;
                }
                if in_bar {
                    inv.push(self.bounds);
                    return;
                }
            }
        }
        // ── 사이드바 스플리터 드래그(폭 조절) ──
        {
            let bx = self.bounds.x;
            let split_x = bx + self.s(self.sidebar_w);
            match *ev {
                InputEvent::MouseDown { x, y, .. }
                    if (x - split_x).abs() <= self.s(4)
                        && y >= self.bounds.y
                        && y < self.bounds.bottom() =>
                {
                    self.split_drag = true;
                    return;
                }
                // ★ 스플리터 hover — 커서가 근처에 오면 하이라이트한다(조절 가능 신호).
                InputEvent::MouseMove { x, y } if !self.split_drag => {
                    let hot = self.wants_col_resize_cursor(x, y);
                    if hot != self.split_hover {
                        self.split_hover = hot;
                        // 실제 색은 tick의 글로우 보간이 **서서히** 올린다(즉시 점등이 아니다).
                        inv.push(self.split_rect());
                    }
                }
                InputEvent::MouseMove { x, .. } if self.split_drag => {
                    let logical = ((x - bx) as f32 / self.scale).round() as i32;
                    let clamped = logical.clamp(110, 320);
                    if clamped != self.sidebar_w {
                        self.sidebar_w = clamped;
                        self.layout(inv);
                        inv.push(self.bounds);
                    }
                    return;
                }
                InputEvent::MouseUp { .. } if self.split_drag => {
                    self.split_drag = false;
                    return;
                }
                _ => {}
            }
        }
        // ── 상단 고정 밴드는 클릭을 **먹는다** ──
        // 밴드는 스크롤해 올라간 행 위에 덮여 있다. 막지 않으면 제목을 눌렀을 뿐인데
        // 보이지도 않는 행의 콤보가 열린다.
        {
            let sw = self.s(self.sidebar_w);
            let crumb = Rect::new(
                self.bounds.x + sw,
                self.bounds.y,
                (self.bounds.w - sw).max(0),
                self.crumb_h(),
            );
            let inside = match *ev {
                InputEvent::MouseDown { x, y, .. } | InputEvent::MouseUp { x, y } => {
                    crumb.contains(Point { x, y })
                }
                _ => false,
            };
            if inside {
                return;
            }
        }

        self.note_mouse(ev);
        // ★ 목록 위 휠은 목록 자신이 스크롤한다(09-01 실기 — 설정 패널이 가로채 가져감).
        //   넘치는 목록만 소비 — 5행 이하는 패널 스크롤에 양보한다.
        if matches!(*ev, InputEvent::Wheel { .. }) {
            let p = self.last_mouse;
            let mut hit = false;
            for row in &mut self.rows {
                if let RowCtl::List(l) = &mut row.ctl {
                    if l.overflows() && l.bounds().contains(p) {
                        l.on_event(ev, inv);
                        hit = true;
                    }
                }
            }
            if hit {
                inv.push(self.bounds);
                return;
            }
        }

        // ── 우측 패널 오버레이 스크롤(세로 전용) — 콤보 열림 중에는 위 캡처가 우선 ──
        {
            let vp = self.right_viewport();
            let (_, ny, consumed) =
                self.bars
                    .on_event(ev, vp, vp.w, self.content_h, 0, self.scroll, self.scale);
            if ny != self.scroll {
                self.scroll = ny;
                self.layout(inv);
                inv.push(self.bounds);
            }
            if consumed {
                inv.push(self.bounds);
                return;
            }
        }

        match *ev {
            InputEvent::MouseDown { x, y, .. } => {
                let p = Point { x, y };
                // 검색/글꼴명 포커스는 클릭 위치 기준(각 컨트롤이 스스로 잡음 + 여기서 블러).
                self.search.set_focused(self.search.bounds().contains(p));
                // ×(지우기) 클릭 처리 — 값이 지워지면 검색 해제 재구성.
                self.search.on_event(ev, inv);
                if self.search.take_changed().is_some() {
                    let q = self.search.text();
                    if q != self.query {
                        self.query = q;
                        self.hist_pos = None;
                        self.rebuild(inv);
                        inv.push(self.bounds);
                        return;
                    }
                }
                // ★ 비밀 행 버튼(09-03) — 눈 = 가림 토글 · 생성 = 2초 무장 후 2차 클릭. 잠긴 행은 건너뛴다(09-04).
                let locked_now: Vec<bool> =
                    self.rows.iter().map(|r| self.is_locked(r.idx)).collect();
                for (r, lock) in self.rows.iter_mut().zip(&locked_now) {
                    if *lock {
                        continue;
                    }
                    let e = &registry()[r.idx];
                    if let (RowCtl::Face(f), SettingKind::Text { secret: true, .. }) =
                        (&mut r.ctl, e.kind)
                    {
                        let (er, rr) = pw_btn_rects(f.bounds());
                        if er.contains(p) {
                            f.set_masked(!f.masked());
                            inv.push(self.bounds);
                            return;
                        }
                        if rr.contains(p) {
                            match self.pw_arm {
                                // 2초 안 재클릭 = 생성 — 새 암호는 **반드시 보이게**(가림 해제).
                                Some(t) if t.elapsed() <= PW_ARM_WINDOW => {
                                    self.pw_arm = None;
                                    f.set_masked(false);
                                    // 값 생성은 호스트(설정 창) 몫이라 가짜 키로 요청만 올린다(sync.test = run 문법).
                                    self.changes
                                        .push(("sync.passphrase.regen", "run".to_string()));
                                }
                                // 첫 클릭 = 무장(빨강) — 실수 클릭으로 암호가 바뀌지 않게.
                                _ => self.pw_arm = Some(std::time::Instant::now()),
                            }
                            inv.push(self.bounds);
                            return;
                        }
                    }
                }
                // ★ 키 이름·복사 글리프 클릭(10-10) = 키 복사 요청(클립보드는 호스트 몫).
                if let Some(key) = self
                    .rows
                    .iter()
                    .find(|r| r.key_rect.get().contains(p))
                    .map(|r| registry()[r.idx].key)
                {
                    self.changes.push(("prefs.copy_key", key.to_string()));
                    inv.push(self.bounds);
                    return;
                }
                // ★ 포커스는 **매 클릭마다 전 컨트롤에 다시 계산**한다. 콤보는 자기 클릭에
                // 스스로 포커스를 켜지만 남의 포커스를 끄지는 못해서, 이걸 빼먹으면
                // 눌러 본 콤보마다 파란 테두리가 남는다(카테고리를 나갔다 오면 재생성돼
                // 사라지던 그 증상 — 사용자 지적 08-09).
                for row in &mut self.rows {
                    match &mut row.ctl {
                        RowCtl::Font { family, size } => {
                            family.set_focused(family.bounds().contains(p));
                            size.set_focused(size.bounds().contains(p));
                        }
                        RowCtl::Pos(g) => g.set_focused(g.bounds().contains(p)),
                        RowCtl::List(l) => {
                            if !l.bounds().contains(p) {
                                l.set_focused(false);
                            }
                        }
                        RowCtl::Face(f) => f.set_focused(f.bounds().contains(p)),
                        RowCtl::Color(c) => {
                            if !c.bounds().contains(p) {
                                c.set_focused(false); // 내부 hex 포커스는 자신의 클릭 처리로
                            }
                        }
                        RowCtl::Combo(c) => c.set_focused(c.bounds().contains(p)),
                        RowCtl::Check(c) => c.set_focused(c.bounds().contains(p)),
                        RowCtl::Act(b) => b.set_focused(b.bounds().contains(p)),
                        RowCtl::Report | RowCtl::Info(_) => {}
                        RowCtl::Devices(rows) => {
                            for r in rows.iter_mut() {
                                r.set_focused(p);
                            }
                        }
                    }
                }
                // 사이드바 트리 — ★트리 영역 안의 클릭만 전달한다(08-22 실기: 우측
                // 노트 줄 클릭이 같은 y의 트리 행을 하이라이트 — 카테고리 전환만
                // bounds로 막고 내부 상태는 무방비였다).
                let before = self.tree.selected_row();
                if self.tree.bounds().contains(p) {
                    self.tree.on_event(ev, inv);
                }
                let after = self.tree.selected_row();
                if self.tree.bounds().contains(p) && after != before
                    || (self.tree.bounds().contains(p) && !self.query.is_empty())
                {
                    if let Some(&sel) = self.cat_map.get(after) {
                        self.selected = sel;
                    }
                    self.hist_pos = None;
                    self.query.clear();
                    self.search.set_text("");
                    self.rebuild(inv);
                    return;
                }
                // 우측 컨트롤들.
                let locked: Vec<bool> = self.rows.iter().map(|r| self.is_locked(r.idx)).collect();
                for (row, lock) in self.rows.iter_mut().zip(locked) {
                    if lock {
                        continue; // 잠긴 설정 — 조건이 갖춰질 때까지 만질 수 없다
                    }
                    match &mut row.ctl {
                        RowCtl::Combo(c) => c.on_event(ev, inv),
                        RowCtl::Check(c) => c.on_event(ev, inv),
                        RowCtl::Font { family, size } => {
                            family.on_event(ev, inv);
                            size.on_event(ev, inv);
                        }
                        RowCtl::Pos(g) => g.on_event(ev, inv),
                        RowCtl::List(l) => l.on_event(ev, inv),
                        RowCtl::Face(f) => f.on_event(ev, inv),
                        RowCtl::Color(c) => c.on_event(ev, inv),
                        RowCtl::Act(b) => b.on_event(ev, inv),
                        RowCtl::Report | RowCtl::Info(_) => {}
                        RowCtl::Devices(rows) => {
                            for r in rows.iter_mut() {
                                r.on_event(ev, inv);
                            }
                        }
                    }
                    if let Some(b) = &mut row.reset {
                        b.on_event(ev, inv);
                    }
                }
                self.drain_changes(inv);
                inv.push(self.bounds);
            }
            InputEvent::MouseUp { .. } => {
                // ★ nexa-ctl 컨트롤은 **전부 "안에서 떼야" 확정**이다(Button·Switch·Combo·PositionPicker·ColorPicker —
                //   10-10 nexa-ui 이관 · 종전 nclip-ctl은 Button·목록만 MouseUp이고 나머지는 MouseDown 완결이었다).
                //   잠긴 행은 MouseDown과 같은 기준으로 건너뛴다(누름이 없었으니 뗌도 무효).
                let locked: Vec<bool> = self.rows.iter().map(|r| self.is_locked(r.idx)).collect();
                for (row, lock) in self.rows.iter_mut().zip(locked) {
                    if lock {
                        continue;
                    }
                    match &mut row.ctl {
                        RowCtl::Combo(c) => c.on_event(ev, inv),
                        RowCtl::Check(c) => c.on_event(ev, inv),
                        RowCtl::Font { family, size } => {
                            family.on_event(ev, inv);
                            size.on_event(ev, inv);
                        }
                        RowCtl::Pos(g) => g.on_event(ev, inv),
                        RowCtl::List(l) => l.on_event(ev, inv),
                        RowCtl::Face(f) => f.on_event(ev, inv),
                        RowCtl::Color(c) => c.on_event(ev, inv),
                        RowCtl::Act(b) => b.on_event(ev, inv),
                        RowCtl::Report | RowCtl::Info(_) => {}
                        RowCtl::Devices(rows) => {
                            for r in rows.iter_mut() {
                                r.on_event(ev, inv);
                            }
                        }
                    }
                    if let Some(b) = &mut row.reset {
                        b.on_event(ev, inv);
                    }
                }
                self.drain_changes(inv);
            }
            // ★ 포커스된 실행 버튼 — Space/Enter = 클릭(종전엔 "기본 타이핑 = 검색"이 삼켜 키보드로는
            //   행위 버튼을 누를 수 없었다 · beep 09-06). 목록 편집 중은 목록이 우선.
            InputEvent::Char { c: ' ', .. }
            | InputEvent::Key {
                key: Key::Enter, ..
            } if !self.any_list_editing()
                && self
                    .rows
                    .iter()
                    .any(|r| matches!(&r.ctl, RowCtl::Act(b) if b.is_focused())) =>
            {
                let locked: Vec<bool> = self.rows.iter().map(|r| self.is_locked(r.idx)).collect();
                for (row, lock) in self.rows.iter_mut().zip(locked) {
                    if let RowCtl::Act(b) = &mut row.ctl {
                        if b.is_focused() && !lock {
                            // nexa-ctl Button은 Enter/Space를 스스로 클릭으로 처리한다.
                            b.on_event(ev, inv);
                        }
                    }
                }
                self.drain_changes(inv);
                inv.push(self.bounds);
            }
            InputEvent::Char { .. } => {
                if self.any_list_editing() {
                    // 목록 행 편집 중 — 문자는 그 입력 상자의 것이다(검색 폴백 금지).
                    self.route_key_to_list(ev, inv);
                } else if self.any_family_focused() {
                    for row in &mut self.rows {
                        match &mut row.ctl {
                            RowCtl::Font { family, .. } if family.is_focused() => {
                                family.on_event(ev, inv);
                            }
                            RowCtl::Face(f) if f.is_focused() => f.on_event(ev, inv),
                            RowCtl::Color(c) if c.hex_focused() => c.on_event(ev, inv),
                            _ => {}
                        }
                    }
                    self.drain_changes(inv);
                } else {
                    // 기본 타이핑 = 검색(포커스 없어도 검색으로 흐른다 — 기존 UX 유지).
                    self.search.set_focused(true);
                    self.search.on_event(ev, inv);
                    let q = self.search.text();
                    if q != self.query {
                        self.query = q;
                        self.hist_pos = None;
                        self.rebuild(inv);
                    }
                }
                inv.push(self.bounds);
            }
            InputEvent::Key { key, .. } => match key {
                // ★ 목록이 키를 원한다(09-01) — 편집 중은 편집 키 전부, 포커스만일 때는
                //   탐색·삭제·확정 키를 그 목록으로. Esc는 편집 취소(편집 중) 또는
                //   포커스 해제(그 외) — 창 닫기로 샐지 않는다.
                Key::Enter | Key::Left | Key::Right | Key::Home | Key::End | Key::Space
                    if self.any_list_editing() =>
                {
                    self.route_key_to_list(ev, inv);
                }
                Key::Up | Key::Down | Key::Delete if self.any_list_wants_keys() => {
                    self.route_key_to_list(ev, inv);
                }
                Key::Enter if self.any_list_wants_keys() => {
                    self.route_key_to_list(ev, inv);
                }
                Key::Escape if self.any_list_editing() => {
                    self.route_key_to_list(ev, inv);
                }
                Key::Escape if self.any_list_wants_keys() => {
                    for row in &mut self.rows {
                        if let RowCtl::List(l) = &mut row.ctl {
                            l.set_focused(false);
                        }
                    }
                    inv.push(self.bounds);
                }
                Key::Escape => {
                    if self.any_family_focused() {
                        for row in &mut self.rows {
                            match &mut row.ctl {
                                RowCtl::Font { family, .. } => family.set_focused(false),
                                RowCtl::Face(f) => f.set_focused(false),
                                RowCtl::Color(c) => c.set_focused(false),
                                _ => {}
                            }
                        }
                        inv.push(self.bounds);
                    } else if self.hist_pos.is_some() {
                        self.hist_pos = None; // 이력 탐색 중 Esc = 탐색만 끝낸다
                    } else {
                        self.back = true;
                    }
                }
                // 글꼴명 입력 중 — Enter(확정)·캐럿 이동을 그 텍스트박스로.
                // (없으면 Face는 take_committed 확정 경로가 영원히 안 밟힌다.)
                Key::Enter | Key::Left | Key::Right | Key::Home | Key::End
                    if self.any_family_focused() =>
                {
                    for row in &mut self.rows {
                        match &mut row.ctl {
                            RowCtl::Font { family, .. } if family.is_focused() => {
                                family.on_event(ev, inv);
                            }
                            RowCtl::Face(f) if f.is_focused() => f.on_event(ev, inv),
                            RowCtl::Color(c) if c.hex_focused() => c.on_event(ev, inv),
                            _ => {}
                        }
                    }
                    self.drain_changes(inv);
                    inv.push(self.bounds);
                }
                Key::Left | Key::Right | Key::Up | Key::Down
                    if self
                        .rows
                        .iter()
                        .any(|r| matches!(&r.ctl, RowCtl::Pos(g) if g.is_focused())) =>
                {
                    for row in &mut self.rows {
                        if let RowCtl::Pos(g) = &mut row.ctl {
                            if g.is_focused() {
                                g.on_event(ev, inv);
                            }
                        }
                    }
                    self.drain_changes(inv);
                    inv.push(self.bounds);
                }
                // ★ 검색 이력(10-10 · nexa-sql 차용): 검색 중 ↑/↓ = 최근 검색어 순환(최근이 먼저).
                Key::Up | Key::Down
                    if !self.history.is_empty()
                        && self.search.is_focused()
                        && (!self.query.is_empty() || self.hist_pos.is_some()) =>
                {
                    let n = self.history.len();
                    let next = match (key, self.hist_pos) {
                        (Key::Up, None) => Some(0),
                        (Key::Up, Some(i)) => Some((i + 1).min(n - 1)),
                        (Key::Down, Some(0) | None) => None,
                        (Key::Down, Some(i)) => Some(i - 1),
                        _ => self.hist_pos,
                    };
                    self.hist_pos = next;
                    let text = next.map_or(String::new(), |i| self.history[i].clone());
                    self.search.set_text(&text);
                    self.query = text;
                    self.rebuild(inv);
                    inv.push(self.bounds);
                }
                // Enter = 검색어를 이력에 기록(중복 제거 · 최근이 앞 · 최대 20) → 호스트가 영속.
                Key::Enter if self.search.is_focused() && !self.query.trim().is_empty() => {
                    let q = self.query.trim().to_string();
                    self.history.retain(|h| *h != q);
                    self.history.insert(0, q);
                    self.history.truncate(HISTORY_MAX);
                    self.hist_pos = None;
                    self.changes.push(("prefs.search", self.history.join("\t")));
                    inv.push(self.bounds);
                }
                Key::Up | Key::Down if self.query.is_empty() => {
                    // 사이드바 카테고리 탐색(검색 중엔 유지).
                    let before = self.tree.selected_row();
                    self.tree.on_event(ev, inv);
                    let after = self.tree.selected_row();
                    if after != before {
                        if let Some(&sel) = self.cat_map.get(after) {
                            self.selected = sel;
                        }
                        self.hist_pos = None;
                        self.rebuild(inv);
                    }
                }
                _ => {}
            },
            _ => {
                // 마우스 이동은 목록·기기 버튼 hover 페이드에도 필요하다(09-01).
                if matches!(*ev, InputEvent::MouseMove { .. }) {
                    for row in &mut self.rows {
                        match &mut row.ctl {
                            RowCtl::List(l) => l.on_event(ev, inv),
                            RowCtl::Devices(rows) => {
                                for r in rows.iter_mut() {
                                    r.on_event(ev, inv);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // 휠 등 — 트리(스크롤바)로.
                self.tree.on_event(ev, inv);
            }
        }
    }

    fn paint(&self, ctx: &mut dyn DrawCtx, theme: &Theme) {
        let lang = current_lang();
        ctx.fill_rect(self.bounds, theme.window_bg); // 카드(panel_bg)가 떠 보이는 바탕(10-10)
        let sw = self.s(self.sidebar_w);
        // 사이드바 배경 + 검색 + 트리 + 경계선.
        ctx.fill_rect(
            Rect::new(self.bounds.x, self.bounds.y, sw, self.bounds.h),
            theme.chrome_bg,
        );
        self.search.paint(ctx, theme);
        self.tree.paint(ctx, theme);
        // ★ 스플리터 — 경계선에서 accent로 **서서히** 밝아진다(글로우 0.0~1.0).
        //
        // ⚠️ **밝기는 세로 전 구간이 균일하다**(사용자 확정 08-26 · [docs/25 §3-7]).
        //   스플리터는 **띠 전체가 하나의 대상**이라 부분이 먼저 밝아지면
        //   "어디를 잡아야 하는가"가 흐려진다 — 한 값으로 **전체를 같이** 올린다.
        let g = self.split_fade.value().clamp(0.0, 1.0);
        if g <= 0.0 {
            ctx.fill_rect(
                Rect::new(self.bounds.x + sw - 1, self.bounds.y, 1, self.bounds.h),
                theme.border,
            );
        } else {
            // 색만 보간한다 — 두께는 진행도와 무관하게 한 번에 정해진다(세로 균일).
            let col = theme.border.lerp(theme.accent, g);
            let w = self.s(2).max(2);
            let x = self.bounds.x + sw - w / 2 - 1;
            ctx.fill_rect(Rect::new(x, self.bounds.y, w, self.bounds.h), col);
        }
        // 하위 섹션 제목(스크롤과 함께 올라간다 — 고정 밴드가 그 위를 덮는다).
        let vp_clip = self.right_viewport();
        // 하위 제목 = 본문(Base)보다 **+1px · 굵게**(사용자 확정 08-11).
        ctx.select_font_sized(FontSlot::Base, true, 1.0);
        for row in &self.rows {
            let Some(sub) = row.head else { continue };
            let hr = Rect::new(row.rect.x, row.rect.y - row.head_h, row.rect.w, row.head_h);
            if hr.bottom() <= vp_clip.y || hr.y >= vp_clip.bottom() {
                continue; // 화면 밖
            }
            let th = ctx.text_height();
            // 상자 **아래쪽**에 붙인다 — 남는 높이가 곧 위 여백이 되어 앞 그룹과 끊긴다.
            ctx.text(
                hr.x + self.s(PAD),
                hr.bottom() - self.s(SUB_HEAD_PAD_B) - th,
                vp_clip,
                tr(lang, sub),
                theme.text,
            );
        }
        // 우측 카드(10-10): 바탕 → 제목·키 → 설명 → 컨트롤(+비밀 행 아이콘) → [초기화]·기본값 → 노트.
        let cpad = self.s(CARD_PAD);
        let title_h = self.s(TITLE_H);
        let desc_gap = self.s(2);
        let ctl_gap = self.s(8);
        let desc_line_h = self.s(DESC_LINE_H);
        let ctl_h = self.s(CTL_H);
        for row in &self.rows {
            let e = &registry()[row.idx];
            let r = row.rect;
            if r.bottom() <= vp_clip.y || r.y >= vp_clip.bottom() {
                continue; // 화면 밖 카드
            }
            ctx.fill_round_rect(r, self.s(6), theme.panel_bg);
            let tx = r.x + cpad;
            let ty = r.y + cpad;
            // 제목(글꼴 영역·목록·보고·기기 목록은 굵게 — 종전 규약 유지).
            let bold_title = matches!(
                row.ctl,
                RowCtl::Font { .. } | RowCtl::List(_) | RowCtl::Report | RowCtl::Devices(_)
            );
            ctx.select_font(FontSlot::Base, bold_title);
            ctx.text(tx, ty, r, tr(lang, e.label), theme.text);
            // 키 이름 + ⧉ — 우상단.
            self.paint_key(ctx, theme, row, r.right() - cpad, ty, r);
            // 설명 — 카드 폭 전체(컨트롤이 아래로 내려가 침범할 것이 없다).
            ctx.select_font(FontSlot::Status, false);
            #[allow(clippy::cast_sign_loss)]
            let lines = wrap_text(
                ctx,
                tr(lang, e.desc),
                row.desc_avail,
                row.desc_lines as usize,
            );
            // 보고·기기 목록 행은 값이 비어 있을 때만 설명을 보인다(종전 규약 — 설명이 "비어 있음" 안내).
            let desc_hidden = match &row.ctl {
                RowCtl::Report => !self
                    .values
                    .get(e.key)
                    .map_or("", String::as_str)
                    .trim()
                    .is_empty(),
                RowCtl::Devices(rows) => !rows.is_empty(),
                _ => false,
            };
            if !desc_hidden {
                for (i, line) in lines.iter().enumerate() {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    let dy = title_h + desc_gap + i as i32 * desc_line_h;
                    ctx.text(tx, ty + dy, r, line, theme.text_dim);
                }
            }
            let cy = ty + title_h + desc_gap + row.desc_lines * desc_line_h + ctl_gap;
            // 컨트롤.
            match &row.ctl {
                RowCtl::Combo(c) => c.paint(ctx, theme),
                RowCtl::Check(c) => c.paint(ctx, theme),
                RowCtl::Act(b) => b.paint(ctx, theme),
                RowCtl::Pos(g) => g.paint(ctx, theme),
                RowCtl::List(l) => l.paint(ctx, theme),
                RowCtl::Face(f) => {
                    f.paint(ctx, theme);
                    // ★ 비밀 행 버튼 — 눈: 보임 = accent · 가림 = 흐림 / 생성: 평소 흐림 · 무장 = 빨강.
                    if matches!(e.kind, SettingKind::Text { secret: true, .. }) {
                        let (er, rr) = pw_btn_rects(f.bounds());
                        let ink = if f.masked() {
                            theme.text_dim
                        } else {
                            theme.accent
                        };
                        tint_icon(&self.pw_eye, PW_EYE_ALPHA, ink.0);
                        draw_icon(&self.pw_eye, er, ctx);
                        let rink = if self.pw_arm.is_some() {
                            theme.danger
                        } else {
                            theme.text_dim
                        };
                        tint_icon(&self.pw_regen, PW_REGEN_ALPHA, rink.0);
                        draw_icon(&self.pw_regen, rr, ctx);
                    }
                }
                RowCtl::Color(c) => c.paint(ctx, theme),
                RowCtl::Font { family, size } => {
                    family.paint(ctx, theme);
                    size.paint(ctx, theme);
                }
                RowCtl::Info(text) => {
                    // 읽기 전용 — 컨트롤 자리에 흐린 글(왼쪽 정렬).
                    ctx.select_font(FontSlot::Status, false);
                    let th = ctx.text_height();
                    let slot = Rect::new(tx, cy, r.w - cpad * 2, ctl_h);
                    ctx.text(tx, cy + (ctl_h - th) / 2, slot, text, theme.text_dim);
                }
                RowCtl::Report => {
                    // 보고 줄들(개행 구분 · 줄 앞 `*` = 강조) — 컨트롤 자리에.
                    ctx.select_font(FontSlot::Status, false);
                    let v = self.values.get(e.key).map_or("", String::as_str);
                    let mut y = cy;
                    for line in v.lines() {
                        let (txt, col) = match line.strip_prefix('*') {
                            Some(rest) => (rest, theme.text),
                            None => (line, theme.text_dim),
                        };
                        ctx.text(tx, y, r, txt, col);
                        y += desc_line_h;
                    }
                }
                RowCtl::Devices(rows) => {
                    ctx.select_font(FontSlot::Status, false);
                    let th = ctx.text_height();
                    for d in rows {
                        let col = if d.emph { theme.text } else { theme.text_dim };
                        #[allow(clippy::cast_sign_loss)]
                        let lines =
                            wrap_text(ctx, &d.text, d.text_w.max(1), d.lines.max(1) as usize);
                        for (i, line) in lines.iter().enumerate() {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                            let ly = d.y + i as i32 * desc_line_h;
                            let clip = Rect::new(tx, ly, d.text_w.max(0), desc_line_h);
                            ctx.text(tx, ly + (desc_line_h - th) / 2, clip, line, col);
                        }
                        if let Some(b) = &d.approve {
                            b.paint(ctx, theme);
                        }
                        if let Some(b) = &d.del {
                            b.paint(ctx, theme);
                        }
                    }
                }
            }
            // [초기화](컨트롤 오른쪽 · 기본값과 다를 때만) + "기본값: …"(우하단 · 한 줄 컨트롤 행만).
            if let Some(b) = &row.reset {
                if b.bounds().w > 0 {
                    b.paint(ctx, theme);
                }
            }
            if let Some(def) = e
                .default_values()
                .into_iter()
                .map(|(_, d)| d)
                .find(|d| !d.is_empty())
            {
                if let Some(cr) = ctl_rect(&row.ctl) {
                    ctx.select_font(FontSlot::Status, false);
                    let txt = nclip_core::tf(Msg::LblDefaultValue, &[&def]);
                    let tw = ctx.text_width(&txt);
                    let th = ctx.text_height();
                    // [초기화]/컨트롤과 겹치면 생략(좁은 카드).
                    let left_edge = row
                        .reset
                        .as_ref()
                        .filter(|b| b.bounds().w > 0)
                        .map_or_else(|| cr.right(), |b| b.bounds().right());
                    let dx = r.right() - cpad - tw;
                    if dx > left_edge + self.s(12) {
                        ctx.text(dx, cy + (ctl_h - th) / 2, r, &txt, theme.text_dim);
                    }
                }
            }
            // 노트(호스트 정보 또는 종속 잠금 안내) — 컨트롤 블록 아래 · 카드 안.
            if let Some((note, tone)) = self.row_note(row.idx) {
                let key = e.key;
                let mono = key == "xfer.approval_window"; // 자동 수락 카운트다운만 고정폭
                ctx.select_font(
                    if mono {
                        FontSlot::Mono
                    } else {
                        FontSlot::Status
                    },
                    false,
                );
                let nh = self.s(NOTE_H);
                let nr = Rect::new(
                    r.x + cpad - self.s(6),
                    r.bottom() - cpad - nh,
                    r.w - cpad * 2 + self.s(12),
                    nh,
                );
                let th = ctx.text_height();
                let color = match tone {
                    NoteTone::Plain => theme.text_dim,
                    NoteTone::Ok => {
                        ctx.fill_round_rect_alpha(nr, self.s(5), theme.ok, 0.14);
                        theme.ok
                    }
                    NoteTone::Warn => {
                        ctx.fill_round_rect_alpha(nr, self.s(5), theme.warn, 0.14);
                        theme.warn
                    }
                    NoteTone::Info => {
                        ctx.fill_round_rect_alpha(nr, self.s(5), theme.accent, 0.10);
                        theme.text
                    }
                };
                ctx.text(nr.x + self.s(6), nr.y + (nr.h - th) / 2, nr, &note, color);
            }
            // 잠긴 카드는 얇은 가림막(그 위 글은 흐려진다 — "지금은 못 만진다").
            if self.is_locked(row.idx) {
                ctx.fill_round_rect_alpha(r, self.s(6), theme.panel_bg, 0.55);
            }
        }
        // 열린 콤보 드롭다운은 맨 위에 다시 그린다(아래 행에 가리지 않게).
        for row in &self.rows {
            match &row.ctl {
                RowCtl::Combo(c) if c.is_open() || c.editing_popup_open() => c.paint(ctx, theme),
                RowCtl::Font { size, .. } if size.is_open() || size.editing_popup_open() => {
                    size.paint(ctx, theme);
                }
                _ => {}
            }
        }
        // 우측 패널 오버레이 스크롤바(맨 위에 겹침 · 세로 전용).
        let vp = self.right_viewport();
        self.bars.paint(
            ctx,
            theme,
            vp,
            vp.w,
            self.content_h,
            0,
            self.scroll,
            self.scale,
        );
        // ── 상단 고정 밴드: 지금 보고 있는 설정의 계층 ──
        // 스크롤해 올라간 섹션 제목이 사라지면, 화면 가운데의 값이 어느 그룹의 것인지
        // 알 수 없다(사용자 지적 08-10). 그래서 **늘 남긴다**.
        // 스크롤 내용을 덮어야 하므로 **맨 마지막에, 불투명하게** 그린다.
        let crumb = Rect::new(
            self.bounds.x + sw,
            self.bounds.y,
            (self.bounds.w - sw).max(0),
            self.crumb_h(),
        );
        ctx.fill_rect(crumb, theme.panel_bg);
        if let Some((cat, sub)) = self.current_group() {
            // 상위 제목 = "그룹 › 카테고리"(10-10) · 본문(Base)보다 **+2px · 굵게**(사용자 확정 08-11).
            ctx.select_font_sized(FontSlot::Base, true, 2.0);
            let th = ctx.text_height();
            let cat_h = self.s(CRUMB_CAT_H);
            // 그룹명 = 카테고리명(일반 › 일반 · 모양 › 모양)이면 한 번만(협업 세션 관찰 ⓑ · 10-10).
            let title = match group_of(cat) {
                Some(g) if tr(lang, g) != tr(lang, cat) => {
                    format!("{} › {}", tr(lang, g), tr(lang, cat))
                }
                _ => tr(lang, cat).to_string(),
            };
            ctx.text(
                crumb.x + self.s(PAD),
                crumb.y + (cat_h - th) / 2,
                crumb,
                &title,
                theme.text,
            );
            // 하위 줄 — 직속 설정 구간이면 비워 둔다(자리는 유지).
            if let Some(sub) = sub {
                // 밴드의 하위 줄은 본문 섹션 제목과 **같은 위계** = 같은 모양으로 보인다.
                ctx.select_font_sized(FontSlot::Base, true, 1.0);
                let sth = ctx.text_height();
                let sub_h = self.s(CRUMB_SUB_H);
                // 한 단 들여써서 "상위 아래"임을 보인다.
                ctx.text(
                    crumb.x + self.s(PAD) + self.s(14),
                    crumb.y + cat_h + (sub_h - sth) / 2,
                    crumb,
                    tr(lang, sub),
                    theme.text_dim,
                );
            }
        }
        // 고급 숨김 배너(밴드 셋째 줄 · 10-10) — "고급 설정 N개 숨김 — 고급 설정을 켜면 보입니다".
        if self.adv_hidden > 0 {
            ctx.select_font(FontSlot::Status, false);
            let th = ctx.text_height();
            let bh = self.s(BANNER_H);
            let by = crumb.bottom() - bh;
            ctx.text(
                crumb.x + self.s(PAD),
                by + (bh - th) / 2,
                crumb,
                &nclip_core::tf(Msg::PrefsAdvancedHidden, &[&self.adv_hidden.to_string()]),
                theme.text_dim,
            );
        }
        ctx.fill_rect(
            Rect::new(crumb.x, crumb.bottom() - 1, crumb.w, 1),
            theme.border,
        );
        // ── 하단 줄(10-10) ──
        let bar = self.bottom_rect();
        ctx.fill_rect(bar, theme.chrome_bg);
        ctx.fill_rect(Rect::new(bar.x, bar.y, bar.w, 1), theme.border);
        self.adv_switch.paint(ctx, theme);
        self.btn_file.paint(ctx, theme);
        self.btn_close.paint(ctx, theme);
        // 텍스트 필드 우클릭 메뉴 — 진짜 최상위(고정 밴드보다도 위 · 08-13 실기:
        // 프로필에서 형제 위젯이 메뉴를 덮던 것과 같은 z순서 계열).
        self.search.paint_popup(ctx, theme);
        for row in &self.rows {
            match &row.ctl {
                RowCtl::Font { family, .. } => family.paint_popup(ctx, theme),
                RowCtl::Face(f) => f.paint_popup(ctx, theme),
                _ => {}
            }
        }
    }
}

/// 행 컨트롤의 자리(글꼴 영역·목록·보고·기기 목록·정보 행은 `None`) — "기본값: …" 배치 기준.
fn ctl_rect(ctl: &RowCtl) -> Option<Rect> {
    match ctl {
        RowCtl::Combo(c) => Some(c.bounds()),
        RowCtl::Check(c) => Some(c.bounds()),
        RowCtl::Act(b) => Some(b.bounds()),
        RowCtl::Pos(g) => Some(g.bounds()),
        RowCtl::Face(f) => Some(f.bounds()),
        RowCtl::Color(c) => Some(c.bounds()),
        RowCtl::Font { .. }
        | RowCtl::List(_)
        | RowCtl::Report
        | RowCtl::Devices(_)
        | RowCtl::Info(_) => None,
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    /// 한 행의 TextBox를 직접 포커스하고 붙여넣기 → 변경 보고(09-04 실기 "암호 붙여넣기가 안 먹는다").
    fn paste_into(key: &'static str, text: &str) -> Vec<(&'static str, String)> {
        let state = SettingsState::with_defaults();
        let mut w = SettingsWidget::new(&state);
        let mut inv = Invalidations::default();
        w.set_bounds(Rect::new(0, 0, 900, 700), &mut inv);
        w.select_category(Msg::CatSync, &mut inv);
        let mut found = false;
        for r in &mut w.rows {
            if registry()[r.idx].key == key {
                if let RowCtl::Face(f) = &mut r.ctl {
                    f.set_focused(true);
                    found = true;
                }
            }
        }
        assert!(found, "행 없음: {key}");
        w.clipboard_paste(text, &mut inv);
        w.take_changes()
    }

    #[test]
    fn paste_reports_handle_and_passphrase() {
        let got = paste_into("sync.handle", "myhandle");
        assert!(
            got.iter()
                .any(|(k, v)| *k == "sync.handle" && v == "myhandle"),
            "{got:?}"
        );
        let got = paste_into("sync.passphrase", "correct horse battery");
        assert!(
            got.iter()
                .any(|(k, v)| *k == "sync.passphrase" && v == "correct horse battery"),
            "{got:?}"
        );
    }
}

#[cfg(test)]
mod validate_tests {
    use super::{validate, SettingsState};
    use nclip_core::Msg;

    /// ★ 숫자 직접 입력의 **범위**를 지킨다 — 0이나 10억을 받으면 상주 앱이 무너진다.
    #[test]
    fn max_items_range_is_enforced() {
        assert!(validate("store.max_items", "1000").is_ok());
        assert!(validate("store.max_items", "10").is_ok(), "하한 포함");
        assert!(validate("store.max_items", "100000").is_ok(), "상한 포함");

        assert_eq!(validate("store.max_items", "9"), Err(Msg::ValItemsRange));
        assert_eq!(
            validate("store.max_items", "100001"),
            Err(Msg::ValItemsRange)
        );
        assert_eq!(validate("store.max_items", "0"), Err(Msg::ValItemsRange));
        // ★ 숫자가 아닌 입력도 같은 경고로 막는다(파일이 손상된 경우 포함).
        assert_eq!(validate("store.max_items", "많이"), Err(Msg::ValItemsRange));
        assert_eq!(validate("store.max_items", ""), Err(Msg::ValItemsRange));
        assert_eq!(validate("store.max_items", "-5"), Err(Msg::ValItemsRange));
    }

    #[test]
    fn tray_count_range_is_enforced() {
        assert!(validate("ui.tray_recent_n", "8").is_ok());
        assert_eq!(
            validate("ui.tray_recent_n", "2"),
            Err(Msg::ValTrayCountRange)
        );
        assert_eq!(
            validate("ui.tray_recent_n", "21"),
            Err(Msg::ValTrayCountRange)
        );
    }

    /// ★ 후보에 없는 **직접 입력값이 화면에서 사라지지 않는다**.
    ///
    /// ⚠️ `Combo::select_value`는 값이 후보에 없으면 **`set_custom_entry`가 먼저
    /// 불려 있어야만** 그 값을 붙잡는다. 순서가 뒤바뀌면 저장된 `2500`이
    /// **빈 콤보**로 뜨고, 사용자는 자기 설정이 사라진 줄 안다.
    /// 빌더 순서가 뒤집히는 회귀를 여기서 잡는다.
    #[test]
    fn custom_number_value_is_shown_not_dropped() {
        use nexa_ctl::controls::{Combo, ComboItem};

        let presets = ["200", "500", "1000"];
        let items: Vec<ComboItem> = presets.iter().map(|v| ComboItem::new(*v, *v)).collect();
        let mut c = Combo::new(items, 0);
        // ★ 이 줄이 select_value보다 **먼저** 와야 한다(빌더와 같은 순서).
        c.set_custom_entry("직접 입력…", "");
        c.select_value("2500");
        assert_eq!(c.selected_value(), "2500", "★ 직접 입력값이 사라졌다");

        // 후보에 있는 값은 그대로 후보 선택으로 잡힌다.
        c.select_value("1000");
        assert_eq!(c.selected_value(), "1000");
    }

    /// 규칙이 없는 키는 통과한다(검증은 **등록된 키에만** 건다).
    #[test]
    fn unregistered_keys_pass() {
        assert!(validate("ui.theme", "dark").is_ok());
        assert!(validate("아무거나", "아무값").is_ok());
    }

    /// ★ 숫자 항목도 **파일에서 되읽힌다** — 문자열 왕복이 깨지면 재시작 때 값이 증발한다.
    #[test]
    fn number_value_round_trips_through_state() {
        let mut st = SettingsState::with_defaults();
        assert_eq!(st.get("store.max_items"), "1000", "기본값");
        assert!(
            st.set_by_name("store.max_items", "5000"),
            "아는 키여야 한다"
        );
        assert_eq!(st.get("store.max_items"), "5000");
        // 후보에 없는 직접 입력값도 그대로 실린다.
        assert!(st.set_by_name("store.max_items", "2500"));
        assert_eq!(st.get("store.max_items"), "2500");
        assert!(st.known_pairs().contains(&("store.max_items", "2500")));
    }
}

/// ★ 설정 카드 개편(10-10 · nexa-beep P2 이식) — 그룹 트리 · 고급 스위치 · 종속 잠금 · 초기화 · 검색 이력 · 자모 검색.
#[cfg(test)]
mod card_tests {
    use super::*;

    fn widget() -> (SettingsWidget, Invalidations) {
        // 테스트는 연타 가드(nexa-ctl 기본 350ms)를 끈다 — 같은 ms에 두 번 누르는 시험이 있다.
        nexa_ctl::controls::button::set_default_click_guard_ms(0);
        let mut w = SettingsWidget::new(&SettingsState::with_defaults());
        let mut inv = Invalidations::default();
        w.set_bounds(Rect::new(0, 0, 900, 700), &mut inv);
        (w, inv)
    }
    /// 클릭 = 누름+뗌 한 쌍(nexa-ctl 컨트롤은 **MouseUp에서 확정**).
    fn click(x: i32, y: i32) -> [InputEvent; 2] {
        [
            InputEvent::MouseDown {
                x,
                y,
                shift: false,
                primary: false,
            },
            InputEvent::MouseUp { x, y },
        ]
    }
    fn key(k: Key) -> InputEvent {
        InputEvent::Key {
            key: k,
            shift: false,
            primary: false,
        }
    }
    fn ch(c: char) -> InputEvent {
        InputEvent::Char { c, now_ms: 0 }
    }
    fn keys_of(w: &SettingsWidget) -> Vec<&'static str> {
        w.rows.iter().map(|r| registry()[r.idx].key).collect()
    }
    fn center(r: Rect) -> (i32, i32) {
        (r.x + r.w / 2, r.y + r.h / 2)
    }

    #[test]
    fn tree_covers_every_category_and_tables_name_real_keys() {
        for e in registry() {
            assert_ne!(
                tree_pos(e.cat).0,
                usize::MAX,
                "트리에 없는 카테고리: {:?}",
                e.cat
            );
        }
        for k in ADVANCED {
            assert!(
                registry().iter().any(|e| e.key == *k),
                "ADVANCED에 없는 키: {k}"
            );
        }
        for (c, p, _) in DEPENDS {
            assert!(
                registry().iter().any(|e| e.key == *c),
                "DEPENDS 자식 없음: {c}"
            );
            assert!(
                registry().iter().any(|e| e.key == *p),
                "DEPENDS 부모 없음: {p}"
            );
        }
        // 표시 순서는 그룹 → 카테고리 순으로 단조.
        let order: Vec<_> = (0..registry().len()).map(display_order).collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted.len(), order.len());
    }

    #[test]
    fn advanced_switch_hides_and_counts() {
        let (mut w, mut inv) = widget();
        w.select_category(Msg::CatCapture, &mut inv);
        let shown = keys_of(&w);
        assert!(!shown.contains(&"cap.cell_pic_rows"), "고급 꺼짐 = 숨김");
        let hidden_expected = registry()
            .iter()
            .filter(|e| e.cat == Msg::CatCapture && is_advanced(e.key))
            .count();
        assert_eq!(w.adv_hidden, hidden_expected);
        assert!(
            w.crumb_h() > w.s(CRUMB_CAT_H) + w.s(CRUMB_SUB_H),
            "배너 줄이 생긴다"
        );
        w.set_advanced(true, &mut inv);
        assert!(keys_of(&w).contains(&"cap.cell_pic_rows"));
        assert_eq!(w.adv_hidden, 0);
    }

    #[test]
    fn group_view_shows_all_categories_with_headers() {
        let (mut w, mut inv) = widget();
        w.selected = TreeSel::Group(1); // 클립보드 그룹
        w.rebuild(&mut inv);
        let cats: Vec<Msg> = w.rows.iter().map(|r| registry()[r.idx].cat).collect();
        for c in [
            Msg::CatCapture,
            Msg::CatPaste,
            Msg::CatSearch,
            Msg::CatStorage,
            Msg::CatPrivacy,
        ] {
            assert!(cats.contains(&c), "{c:?} 없음");
        }
        // 카테고리가 바뀌는 첫 행마다 카테고리 제목이 붙는다.
        let heads = w.rows.iter().filter(|r| r.head.is_some()).count();
        assert!(heads >= 5, "제목 {heads}");
        assert_eq!(w.rows[0].head, Some(Msg::CatCapture));
    }

    #[test]
    fn depends_locks_child_until_parent_satisfied() {
        let (mut w, mut inv) = widget();
        w.set_advanced(true, &mut inv);
        w.select_category(Msg::CatSync, &mut inv);
        let idx = w
            .rows
            .iter()
            .position(|r| registry()[r.idx].key == "sync.files_max")
            .expect("sync.files_max 행");
        let ridx = w.rows[idx].idx;
        w.set_value("sync.files", "on", &mut inv);
        assert!(!w.is_locked(ridx));
        w.set_value("sync.files", "off", &mut inv);
        assert!(w.is_locked(ridx), "부모가 꺼지면 잠긴다");
        let (note, _) = w.row_note(ridx).expect("잠금 안내");
        assert!(
            note.contains(tr(current_lang(), Msg::SetSyncFiles)),
            "{note}"
        );
        w.set_value("sync.files", "on", &mut inv);
        assert!(!w.is_locked(ridx));
    }

    #[test]
    fn reset_button_appears_when_modified_and_restores_default() {
        let (mut w, mut inv) = widget();
        w.select_category(Msg::CatGeneral, &mut inv);
        let find = |w: &SettingsWidget| {
            w.rows
                .iter()
                .position(|r| registry()[r.idx].key == "ui.close_to_tray")
                .expect("ui.close_to_tray 행")
        };
        let i = find(&w);
        assert_eq!(
            w.rows[i].reset.as_ref().map(|b| b.bounds().w),
            Some(0),
            "기본값이면 숨김"
        );
        w.set_value("ui.close_to_tray", "off", &mut inv);
        w.layout(&mut inv);
        let i = find(&w);
        let rb = w.rows[i]
            .reset
            .as_ref()
            .map(|b| b.bounds())
            .expect("초기화 버튼");
        assert!(rb.w > 0, "값이 바뀌면 보인다");
        let (x, y) = center(rb);
        for e in click(x, y) {
            w.on_event(&e, &mut inv);
        }
        let ch = w.take_changes();
        assert!(
            ch.iter()
                .any(|(k, v)| *k == "ui.close_to_tray" && v == "on"),
            "초기화 = 기본값 보고: {ch:?}"
        );
    }

    #[test]
    fn search_history_records_on_enter_and_cycles() {
        let (mut w, mut inv) = widget();
        for c in "theme".chars() {
            w.on_event(&ch(c), &mut inv);
        }
        assert_eq!(w.query, "theme");
        w.on_event(&key(Key::Enter), &mut inv);
        assert_eq!(w.history, vec!["theme".to_string()]);
        let ch_ = w.take_changes();
        assert!(ch_
            .iter()
            .any(|(k, v)| *k == "prefs.search" && v == "theme"));
        // 새 검색어 입력 중 ↑ = 이력 순환.
        w.on_event(&key(Key::Escape), &mut inv);
        w.query.clear();
        w.search.set_text("");
        w.on_event(&ch('x'), &mut inv);
        assert_eq!(w.query, "x");
        w.on_event(&key(Key::Up), &mut inv);
        assert_eq!(w.query, "theme");
        w.on_event(&key(Key::Down), &mut inv);
        assert_eq!(w.query, "");
    }

    #[test]
    fn jamo_search_matches_composing_hangul() {
        let e = registry()
            .iter()
            .find(|e| e.key == "ui.theme")
            .expect("ui.theme");
        assert!(entry_matches(e, &tokens("테")));
        assert!(entry_matches(e, &tokens("ㅌ")), "조합 중 자모도 맞는다");
        assert!(entry_matches(e, &tokens("theme")));
        assert!(entry_matches(e, &tokens("ui.theme")), "키 이름도 검색 대상");
        assert!(!entry_matches(e, &tokens("ㅍㅍㅍ")));
    }

    #[test]
    fn bottom_bar_close_and_advanced_switch() {
        let (mut w, mut inv) = widget();
        let (x, y) = center(w.btn_close.bounds());
        for e in click(x, y) {
            w.on_event(&e, &mut inv);
        }
        assert!(w.take_back(), "[닫기] = 닫기 요청");
        let (x, y) = center(w.adv_switch.bounds());
        for e in click(x, y) {
            w.on_event(&e, &mut inv);
        }
        assert!(w.advanced, "스위치 = 고급 켜짐");
        let ch_ = w.take_changes();
        assert!(
            ch_.iter()
                .any(|(k, v)| *k == "ui.prefs_advanced" && v == "on"),
            "{ch_:?}"
        );
    }

    #[test]
    fn key_click_requests_copy() {
        let (mut w, mut inv) = widget();
        // key_rect는 페인트가 채운다 — 시험에선 첫 행의 자리를 손으로 넣는다.
        let r = w.rows[0].rect;
        let kr = Rect::new(r.right() - 60, r.y + 4, 50, 16);
        w.rows[0].key_rect.set(kr);
        let key0 = registry()[w.rows[0].idx].key;
        let (x, y) = center(kr);
        w.on_event(&click(x, y)[0], &mut inv);
        let ch_ = w.take_changes();
        assert!(
            ch_.iter().any(|(k, v)| *k == "prefs.copy_key" && v == key0),
            "{ch_:?}"
        );
    }
}
