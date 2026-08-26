//! ★ **디자인 토큰** — [docs/25](../../../docs/25-design-system.md)의 수치를 코드로 못 박는다.
//!
//! **Material에서 규칙을, macOS에서 느낌을**([DR-27](../../../docs/10-decision-record.md)).
//! Material은 *얼마나 띄우고 얼마나 크게*를, macOS는 *얼마나 둥글고 부드러운가*를 정한다.
//!
//! ## 왜 상수로 두는가
//!
//! beep과의 격차 진단([docs/25 §1](../../../docs/25-design-system.md))에서 *"간격 리듬이 산발"* 이
//! 나왔다. **"3px만 더"가 쌓이면 리듬이 무너진다** — 눈대중을 없애려면 값에 이름을 줘야 한다.

use crate::theme::Color;

/// 간격 — **4px 그리드**. 이 여섯 개 밖의 값을 쓰지 않는다.
pub mod space {
    /// 4 — 아이콘과 글자 사이 같은 최소 간격.
    pub const XS: i32 = 4;
    /// 8 — 컨트롤 사이.
    pub const S: i32 = 8;
    /// 12 — 컨트롤 안쪽 좌우 여백 · 창 가장자리.
    pub const M: i32 = 12;
    /// 16 — 묶음 사이.
    pub const L: i32 = 16;
    /// 24 — 섹션 사이.
    pub const XL: i32 = 24;
    /// 32 — 큰 구역 사이.
    pub const XXL: i32 = 32;

    /// 4의 배수로 맞춘다 — 계산 결과가 리듬에서 벗어나지 않게.
    #[must_use]
    pub const fn snap(v: i32) -> i32 {
        (v + 2) / 4 * 4
    }
}

/// 코너 반경 — **macOS 쪽**(넉넉하게).
pub mod radius {
    /// 12 — 창·팝업.
    pub const WINDOW: i32 = 12;
    /// 10 — 패널·카드·시트.
    pub const PANEL: i32 = 10;
    /// 6 — 버튼·필드·콤보.
    pub const CONTROL: i32 = 6;
    /// 배지·칩(pill) — 높이의 절반을 쓰라는 뜻의 큰 값.
    pub const PILL: i32 = 999;
}

/// 타입 스케일 — 1.0배 기준 크기(px)와 굵기.
pub mod type_scale {
    /// 창 제목 · 설정 카테고리.
    pub const TITLE: (f32, bool) = (15.0, true);
    /// ★ 목록 항목 본문 — **가장 많이 쓰인다**.
    pub const BODY: (f32, bool) = (13.0, false);
    /// 버튼 · 툴바.
    pub const LABEL: (f32, bool) = (12.0, false);
    /// 출처 앱 · 시각 · 설명.
    pub const CAPTION: (f32, bool) = (11.0, false);
    /// 코드·해시·경로(고정폭 슬롯과 함께).
    pub const MONO: (f32, bool) = (12.5, false);

    /// 행 높이 = 크기 × 1.45, **4의 배수로 스냅**.
    #[must_use]
    pub fn line_height(size_px: f32) -> i32 {
        super::space::snap((size_px * 1.45).round() as i32)
    }
}

/// ★ **상태 레이어** — Material의 핵심 아이디어.
///
/// **색을 새로 만들지 않는다.** 기존 색 위에 **같은 색을 알파로 덮는다**
/// ([docs/23 A-5](../../../docs/23-alpha-rendering.md)).
/// 이 표 하나가 *"손에 닿는 느낌"* 의 대부분이고, 구현 비용은 `fill_rect_alpha` 한 줄이다.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum State {
    /// 평상.
    #[default]
    Rest,
    /// 커서가 올라와 있다.
    Hover,
    /// 눌려 있다.
    Pressed,
    /// 선택돼 있다.
    Selected,
    /// 선택 + 커서.
    SelectedHover,
    /// 비활성(전경을 흐리게 — 배경 오버레이가 아니다).
    Disabled,
}

impl State {
    /// 배경 위에 덮을 오버레이 불투명도. `Disabled`는 배경을 안 덮는다(0.0).
    #[must_use]
    pub fn overlay_alpha(self) -> f32 {
        match self {
            State::Rest => 0.0,
            State::Hover => 0.08,
            State::Pressed => 0.12,
            State::Selected => 0.16,
            State::SelectedHover => 0.20,
            State::Disabled => 0.0,
        }
    }

    /// 전경(글자·아이콘) 불투명도 — 비활성만 낮춘다.
    #[must_use]
    pub fn content_alpha(self) -> f32 {
        if matches!(self, State::Disabled) {
            0.38
        } else {
            1.0
        }
    }

    /// 커서·눌림을 상태로 합친다(위젯이 매번 분기하지 않게).
    #[must_use]
    pub fn of(selected: bool, hover: bool, pressed: bool, enabled: bool) -> State {
        if !enabled {
            State::Disabled
        } else if pressed {
            State::Pressed
        } else if selected && hover {
            State::SelectedHover
        } else if selected {
            State::Selected
        } else if hover {
            State::Hover
        } else {
            State::Rest
        }
    }
}

/// 엘리베이션 — **그림자로 층을 말한다**.
///
/// 두 겹으로 그린다(가까운 진한 것 + 먼 옅은 것) — **한 겹은 딱딱해 보인다**.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Elevation {
    /// 창 배경 — 그림자 없음.
    #[default]
    Flat,
    /// 목록 카드 · 툴바.
    Low,
    /// 팝오버 · 드롭다운.
    Mid,
    /// 시트 · 모달.
    High,
}

/// 그림자 한 겹 — `(y 오프셋, 번짐, 알파)`.
pub type ShadowLayer = (i32, i32, f32);

impl Elevation {
    /// 이 층이 그릴 그림자 겹들(먼 것 → 가까운 것 순 — **먼저 그린 위에 덮는다**).
    #[must_use]
    pub fn layers(self) -> &'static [ShadowLayer] {
        match self {
            Elevation::Flat => &[],
            Elevation::Low => &[(2, 4, 0.06), (1, 2, 0.10)],
            Elevation::Mid => &[(6, 12, 0.10), (2, 4, 0.16)],
            Elevation::High => &[(12, 24, 0.14), (4, 8, 0.22)],
        }
    }
}

/// 모션 — 지속(ms). ★ **팝업은 120ms를 넘기지 않는다**(3초 예산에서 애니메이션은 순수 비용).
pub mod motion {
    /// 상태 변화(hover·press).
    pub const STATE_MS: u32 = 90;
    /// 팝업 등장.
    pub const POPUP_MS: u32 = 120;
    /// 패널 열기/닫기.
    pub const PANEL_MS: u32 = 160;
    /// ★ **스플리터 글로우 페이드인**(사용자 요청 08-26 — *"서서히 밝아지는 듯하게"*).
    ///
    /// 다른 상태 전이(90ms)보다 훨씬 길다. **의도된 예외**다 —
    /// 스플리터는 *"여기 잡을 수 있다"* 를 **조용히 알리는** 장치라
    /// 빠르게 번쩍이면 오히려 시선을 뺏는다. 나가는 쪽은 빠르게([`SPLITTER_OUT_MS`]).
    pub const SPLITTER_IN_MS: u32 = 1000;
    /// 스플리터 글로우 페이드아웃 — 들어올 때보다 **빨리 꺼진다**(잔상 방지).
    pub const SPLITTER_OUT_MS: u32 = 220;

    /// `reduce_motion`이면 전부 0으로 — 접근성 설정을 존중한다.
    #[must_use]
    pub const fn effective(ms: u32, reduce_motion: bool) -> u32 {
        if reduce_motion {
            0
        } else {
            ms
        }
    }
}

/// 상태 오버레이에 쓸 색을 고른다 — **강조 계열이면 accent, 아니면 전경색**.
///
/// 색을 새로 만들지 않는다는 규칙([docs/25 §3-4])의 구현 지점이다.
#[must_use]
pub fn overlay_color(theme: &crate::theme::Theme, on_accent: bool) -> Color {
    if on_accent {
        theme.accent
    } else {
        theme.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ 간격은 전부 4의 배수여야 한다 — 리듬의 근거.
    #[test]
    fn spacing_is_on_four_grid() {
        for v in [
            space::XS,
            space::S,
            space::M,
            space::L,
            space::XL,
            space::XXL,
        ] {
            assert_eq!(v % 4, 0, "{v}는 4의 배수가 아니다");
        }
    }

    #[test]
    fn snap_rounds_to_grid() {
        assert_eq!(space::snap(1), 0);
        assert_eq!(space::snap(2), 4);
        assert_eq!(space::snap(13), 12);
        assert_eq!(space::snap(14), 16);
    }

    /// ★ 상태가 진해질수록 오버레이도 진해야 한다 — 역전되면 눌린 게 덜 눌려 보인다.
    #[test]
    fn overlay_alpha_is_monotonic() {
        let a = State::Rest.overlay_alpha();
        let b = State::Hover.overlay_alpha();
        let c = State::Pressed.overlay_alpha();
        let d = State::Selected.overlay_alpha();
        let e = State::SelectedHover.overlay_alpha();
        assert!(a < b && b < c, "rest < hover < pressed");
        assert!(d < e, "selected < selected+hover");
    }

    #[test]
    fn disabled_dims_content_not_background() {
        assert_eq!(State::Disabled.overlay_alpha(), 0.0);
        assert!(State::Disabled.content_alpha() < 1.0);
    }

    /// 상태 합성 우선순위 — 비활성이 가장 세고, 눌림이 선택보다 앞선다.
    #[test]
    fn state_of_priority() {
        assert_eq!(State::of(true, true, true, false), State::Disabled);
        assert_eq!(State::of(true, true, true, true), State::Pressed);
        assert_eq!(State::of(true, true, false, true), State::SelectedHover);
        assert_eq!(State::of(true, false, false, true), State::Selected);
        assert_eq!(State::of(false, true, false, true), State::Hover);
        assert_eq!(State::of(false, false, false, true), State::Rest);
    }

    /// ★ 그림자는 **두 겹**이다 — 한 겹은 딱딱해 보인다.
    #[test]
    fn elevation_has_two_layers_when_raised() {
        assert!(Elevation::Flat.layers().is_empty());
        for e in [Elevation::Low, Elevation::Mid, Elevation::High] {
            assert_eq!(e.layers().len(), 2, "{e:?}는 두 겹이어야 한다");
        }
    }

    /// 층이 높을수록 더 멀리·더 진하게.
    #[test]
    fn elevation_grows_with_level() {
        let near = |e: Elevation| e.layers().last().copied().unwrap_or((0, 0, 0.0));
        assert!(near(Elevation::Mid).2 > near(Elevation::Low).2);
        assert!(near(Elevation::High).2 > near(Elevation::Mid).2);
    }

    /// ★ 팝업 애니메이션은 120ms를 넘지 않는다(3초 예산 보호).
    #[test]
    fn popup_motion_is_capped() {
        const _: () = assert!(
            motion::POPUP_MS <= 120,
            "팝업 애니메이션이 3초 예산을 잠식한다"
        );
        assert_eq!(
            motion::effective(motion::POPUP_MS, true),
            0,
            "reduce_motion이면 0"
        );
    }

    /// 행 높이도 4의 배수로 떨어진다.
    #[test]
    fn line_height_snaps() {
        for (size, _) in [type_scale::BODY, type_scale::TITLE, type_scale::CAPTION] {
            assert_eq!(type_scale::line_height(size) % 4, 0);
        }
    }
}
