//! 붙여넣기 포트 — ★ **이 제품의 존재 이유가 걸린 계층**(FR-P-2 · [K-1 리스크](../../../docs/02-roadmap.md)).
//!
//! ## 왜 어려운가
//!
//! 팝업이 뜨면 **포커스가 우리에게 온다**. 사용자가 항목을 고르면 우리는
//! **원래 있던 창으로 포커스를 돌려주고 붙여넣기 키를 넣어야** 한다. 이 왕복이
//! 3-OS에서 각각 다르고, **권한이 걸린다**([docs/20 §3-4](../../../docs/20-implementation-spec.md)).
//!
//! ```text
//! ① 단축키 수신
//! ② ★ 직전 포그라운드 창/앱을 기억      ← 팝업을 띄우기 "전"
//! ③ 팝업 표시(포커스 획득) · 사용자 선택
//! ④ 팝업 숨김
//! ⑤ ★ ②를 다시 활성화
//! ⑥ 붙여넣기 키 주입(Ctrl+V / ⌘V)
//! ```
//!
//! ## 실패는 조용하면 안 된다
//!
//! 권한이 없거나(macOS 손쉬운 사용) 프로토콜이 없으면(Wayland) **주입을 못 한다**.
//! 그때는 [`PasteCapability`]로 **미리** 알리고, 실제 동작은 *"클립보드에만 올림"* 으로
//! **정직하게 강등**한다(FR-P-1). 사용자가 왜 안 되는지 알아야 한다.

/// 붙여넣기 계층이 이 환경에서 무엇을 할 수 있는가.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PasteCapability {
    /// 포커스 복원 + 키 주입까지 가능.
    Full {
        /// 구현 이름(진단 표시용 — 예: `win32-sendinput` · `mac-cgevent`).
        backend: &'static str,
    },
    /// ★ **권한만 주면 된다** — 기능은 있으나 사용자가 허용해야 한다.
    /// macOS 손쉬운 사용(Accessibility)이 이 경우다.
    NeedsPermission {
        /// 구현 이름.
        backend: &'static str,
        /// 사용자에게 보여줄 안내(어느 설정을 켜야 하는지).
        hint: &'static str,
    },
    /// ★ **구조적으로 불가능** — 클립보드 적재까지만 한다(FR-P-1 강등).
    ClipboardOnly {
        /// 왜 불가능한지.
        reason: PasteUnsupported,
    },
}

impl PasteCapability {
    /// 키 주입까지 실제로 시도할 수 있는가(권한 대기 상태는 **아직 아니다**).
    #[must_use]
    pub fn can_inject(&self) -> bool {
        matches!(self, PasteCapability::Full { .. })
    }
}

/// 주입이 불가능한 이유.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PasteUnsupported {
    /// Wayland — 합성기 밖에서 키를 넣는 표준이 없다.
    WaylandNoInjection,
    /// 표시 서버에 연결할 수 없다.
    NoDisplayServer,
    /// 이 타깃은 아직 구현되지 않았다.
    NotImplemented,
}

/// 붙여넣기 실패.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PasteError {
    /// 이 환경에서는 불가능하다 — 호출자는 클립보드 적재로 강등한다.
    Unsupported(PasteUnsupported),
    /// 권한이 없다(사용자 안내 대상).
    PermissionDenied {
        /// 어느 설정을 켜야 하는지.
        hint: &'static str,
    },
    /// 기억해 둔 창이 이미 사라졌다(사용자가 닫았다).
    TargetGone,
    /// OS 호출 실패(진단 문자열).
    Os(String),
}

/// 붙여넣을 형식 — [docs/12 §5](../../../docs/12-clipboard-formats.md) ·
/// [docs/26 §4-5](../../../docs/26-file-content-sharing.md).
///
/// ## ★ 네 모드가 전부 같은 기계다 — **고르는 게 아니라 "빼는" 것**
///
/// 우리는 대상 앱을 **탐지하지 않는다**. 붙여넣기 직전에 **클립보드에 올리는 표현을 좁혀서**
/// 받는 앱이 고를 수밖에 없게 만든다. 앱 이름 목록을 유지할 필요가 없고,
/// 원격 데스크톱·가상 머신·Electron 앱에서도 어긋나지 않는다.
///
/// | 모드 | 키 | 남기는 표현 |
/// |---|---|---|
/// | [`Original`](PasteAs::Original) | `Enter` | 전부 |
/// | [`Plain`](PasteAs::Plain) | `⇧Enter` | 평문 하나 |
/// | [`Object`](PasteAs::Object) | `⌘/Ctrl+Enter` | 파일·개체 표현만 |
/// | [`PathOnly`](PasteAs::PathOnly) | `⌥/Alt+Enter` | 경로 텍스트만 |
///
/// ⚠️ **모드는 사용자의 요청이지 보증이 아니다** — 원격 파일이 용량 상한을 넘으면
/// [`Object`](PasteAs::Object)를 요청해도 경로만 나간다([docs/26 §4-4]).
/// ★ 사용자의 의사보다 *"실패할 약속을 하지 않는다"* 가 먼저다.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum PasteAs {
    /// 원본 그대로 — 클립보드에 올린 표현 전부를 그대로 둔다.
    #[default]
    Original,
    /// 평문으로 — 평문 표현 하나만 올린다(FR-P-3).
    Plain,
    /// ★ **객체로** — 파일·개체 표현만 남긴다.
    ///
    /// 텍스트를 빼면 PowerPoint·Word가 **파일 개체로 받을 수밖에 없다**.
    /// 둘 다 할 수 있는 앱에서 *"경로 글자로 붙었다"* 를 막는다.
    Object,
    /// ★ **경로만** — 경로 텍스트만 남긴다.
    ///
    /// 파일 표현을 빼므로 ★ **원격 파일 내용을 끌어오지 않는다**(회선이 느리거나 종량일 때).
    PathOnly,
}

impl PasteAs {
    /// 이 종류의 항목에서 **뜻이 있는 모드들** — 순서는 화면에 보여줄 순서.
    ///
    /// ★ **힌트 줄·우클릭 메뉴·키 처리가 전부 이 한 곳을 본다.**
    /// 셋이 따로 판단하면 *"메뉴에는 있는데 키는 안 먹는"* 상태가 반드시 생긴다.
    ///
    /// ⚠️ 이미지 항목에 *"경로만"* 을 띄우는 것은 **거짓말**이다 — 해당 없는 모드는 빼야 한다.
    #[must_use]
    pub fn applicable(kind: crate::ClipKind) -> &'static [PasteAs] {
        use crate::ClipKind as K;
        match kind {
            // 파일만 개체/경로 구분이 뜻을 가진다.
            K::Files => &[PasteAs::Original, PasteAs::Object, PasteAs::PathOnly],
            // 서식이 있는 것은 평문으로 낮출 수 있다.
            K::RichText => &[PasteAs::Original, PasteAs::Plain],
            // 평문·색은 이미 평문이라 낮출 곳이 없고, 이미지는 텍스트가 없다.
            K::Text | K::Color | K::Image => &[PasteAs::Original],
        }
    }

    /// i18n 라벨 키.
    #[must_use]
    pub fn label(self) -> crate::Msg {
        match self {
            PasteAs::Original => crate::Msg::PasteOriginal,
            PasteAs::Plain => crate::Msg::PastePlain,
            PasteAs::Object => crate::Msg::PasteObject,
            PasteAs::PathOnly => crate::Msg::PastePathOnly,
        }
    }
}

/// ★ **붙여넣기 포트**.
///
/// 구현은 `nclip-plat`에 있고, 본체가 조립 시점에 주입한다.
pub trait PasteInjector: core::fmt::Debug {
    /// 이 환경에서 무엇이 가능한지. **팝업을 띄우기 전에** 물어볼 수 있어야 한다.
    fn capability(&self) -> PasteCapability;

    /// ② 지금 포그라운드에 있는 창/앱을 기억한다. ★ **팝업을 띄우기 전에** 부른다.
    ///
    /// 기억할 대상이 없으면(포그라운드 없음) `false`.
    fn capture_focus(&mut self) -> bool;

    /// ⑤+⑥ 기억해 둔 대상을 다시 활성화하고 붙여넣기 키를 넣는다.
    ///
    /// # Errors
    /// 권한 부재·대상 소실·OS 실패 시 [`PasteError`]. 호출자는 **클립보드 적재로 강등**한다.
    fn restore_and_paste(&mut self, as_: PasteAs) -> Result<(), PasteError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_full_can_inject() {
        assert!(PasteCapability::Full { backend: "x" }.can_inject());
        // ★ 권한 대기는 "아직 못 한다" — 낙관적으로 시도하면 조용히 실패한다.
        assert!(!PasteCapability::NeedsPermission {
            backend: "x",
            hint: "y"
        }
        .can_inject());
        assert!(!PasteCapability::ClipboardOnly {
            reason: PasteUnsupported::WaylandNoInjection
        }
        .can_inject());
    }

    #[test]
    fn paste_as_defaults_to_original() {
        // 정보를 잃지 않는 쪽이 기본이다(D-31).
        assert_eq!(PasteAs::default(), PasteAs::Original);
    }

    /// ★ 어떤 종류든 **원본은 항상 첫 번째**다 — 기본 동작이 목록 맨 앞에 있어야 한다.
    #[test]
    fn original_is_always_first_and_present() {
        for k in [
            crate::ClipKind::Text,
            crate::ClipKind::RichText,
            crate::ClipKind::Image,
            crate::ClipKind::Files,
            crate::ClipKind::Color,
        ] {
            let modes = PasteAs::applicable(k);
            assert_eq!(modes.first(), Some(&PasteAs::Original), "{k:?}");
        }
    }

    /// ★ 해당 없는 모드를 띄우면 거짓말이다 — 이미지에 "경로만"이 뜨면 안 된다.
    #[test]
    fn modes_do_not_lie_about_the_item() {
        let img = PasteAs::applicable(crate::ClipKind::Image);
        assert!(!img.contains(&PasteAs::PathOnly), "이미지에 경로는 없다");
        assert!(!img.contains(&PasteAs::Plain), "이미지에 평문은 없다");

        let text = PasteAs::applicable(crate::ClipKind::Text);
        assert!(!text.contains(&PasteAs::Object), "평문에 개체는 없다");

        // 평문은 이미 평문이라 "평문으로 낮추기"가 뜻이 없다.
        assert!(
            !text.contains(&PasteAs::Plain),
            "평문을 평문으로 낮출 수 없다"
        );
    }

    /// 파일에서만 개체/경로 구분이 뜻을 가진다(이 기능이 있는 이유).
    #[test]
    fn files_offer_object_and_path() {
        let f = PasteAs::applicable(crate::ClipKind::Files);
        assert!(f.contains(&PasteAs::Object));
        assert!(f.contains(&PasteAs::PathOnly));
    }

    /// 모드마다 라벨이 있고 서로 다르다(하나라도 겹치면 메뉴가 애매해진다).
    #[test]
    fn every_mode_has_a_distinct_label() {
        let all = [
            PasteAs::Original,
            PasteAs::Plain,
            PasteAs::Object,
            PasteAs::PathOnly,
        ];
        let labels: Vec<_> = all.iter().map(|m| m.label()).collect();
        for (i, a) in labels.iter().enumerate() {
            for b in labels.iter().skip(i + 1) {
                assert_ne!(a, b, "라벨이 겹친다");
            }
        }
    }
}
