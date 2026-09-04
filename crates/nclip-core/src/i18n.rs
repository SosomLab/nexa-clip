//! i18n — **프로세스 전역 로케일 + 메시지 카탈로그**(영어 기본 · 한/중/일 언어팩).
//!
//! ★ **형태를 `nexa-beep` `nbeep-core::i18n`과 동형으로 맞춘다**([docs/13 §5](../../../docs/13-ui-reuse-from-beep.md)) —
//! 이식해 오는 `settings.rs`·컨트롤이 `tr(lang, Msg::…)` 계약에 의존하므로, 같은 모양이면
//! **`use` 경로만 바꾸면 된다**(DR-16).
//!
//! 외부 i18n 크레이트를 쓰지 않는다(DR-8). 문자열은 전부 `&'static str`이라 힙 할당·파일
//! 로드가 없다 — 예산 게이트(DR-9)에 무해하다.
//!
//! ## 카탈로그 형태 — "한 키 = 한 줄 4언어"
//!
//! [`Msg`]의 각 변형이 `[en, ko, zh, ja]`를 한 줄로 갖는다([`Msg::row`]). 한 줄에 모여 있어
//! **누락·불일치를 리뷰에서 바로 본다**. 새 UI 문자열 = 변형 1개 + 줄 1개.

use core::sync::atomic::{AtomicU8, Ordering};

/// 지원 언어 — **영어 기본** · 4개 기본 관리(사용자 확정 2026-08-26).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Lang {
    /// 영어(기본·폴백).
    #[default]
    En,
    /// 한국어.
    Ko,
    /// 중국어(간체).
    Zh,
    /// 일본어.
    Ja,
}

impl Lang {
    /// 전 언어(설정 콤보·순회용).
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Ko, Lang::Zh, Lang::Ja];

    /// 값 코드(설정 저장·복원 계약).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
        }
    }

    /// 코드 → 언어(미지 코드는 `None` — 호출자가 기본으로 폴백).
    #[must_use]
    pub fn from_code(s: &str) -> Option<Lang> {
        match s {
            "en" => Some(Lang::En),
            "ko" => Some(Lang::Ko),
            "zh" => Some(Lang::Zh),
            "ja" => Some(Lang::Ja),
            _ => None,
        }
    }

    /// 카탈로그 열 번호([`Msg::row`]의 인덱스).
    #[must_use]
    pub fn column(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::Ko => 1,
            Lang::Zh => 2,
            Lang::Ja => 3,
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

/// 현재 언어를 바꾼다(프로세스 전역).
pub fn set_lang(lang: Lang) {
    LANG.store(lang.column() as u8, Ordering::Relaxed);
}

/// 현재 언어.
#[must_use]
pub fn current_lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Ko,
        2 => Lang::Zh,
        3 => Lang::Ja,
        _ => Lang::En,
    }
}

/// UI 문자열 키. **화면에 나가는 문자열은 전부 여기를 거친다** — 하드코딩 금지.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum Msg {
    /// 앱 이름.
    AppName,
    /// 검색 입력 안내문.
    SearchPlaceholder,
    /// 항목이 하나도 없을 때.
    EmptyHistory,
    /// 검색 결과 없음.
    NoMatches,

    // ── 타입 라벨 ─────────────────────────────
    /// 텍스트 항목.
    KindText,
    /// 서식 있는 텍스트.
    KindRichText,
    /// 이미지 항목.
    KindImage,
    /// 파일·폴더 목록 항목.
    KindFiles,
    /// 색상 항목.
    KindColor,

    // ── 보기 모드(FR-U-14) ────────────────────
    /// 일반 보기(서식·이미지를 그대로).
    ViewRich,
    /// 간략 보기(1행 + 작은 썸네일) — 기본값.
    ViewCompact,
    /// 한 줄 보기(평문 1줄).
    ViewPlain,

    // ── 동작 ──────────────────────────────────
    /// 복사(클립보드에 올림).
    ActionCopy,
    /// 종류 — 앱 고유 개체(도형·차트).
    KindObject,
    /// 보관 개수 입력 범위 경고.
    ValItemsRange,
    /// 트레이 최근 개수 입력 범위 경고.
    ValTrayCountRange,
    /// 붙여넣기 모드 — 원본 그대로.
    PasteOriginal,
    /// 붙여넣기 모드 — 평문으로.
    PastePlain,
    /// ★ 붙여넣기 모드 — **객체로**(파일·개체 표현만).
    PasteObject,
    /// ★ 붙여넣기 모드 — **경로만**(원격 내용을 끌어오지 않는다).
    PastePathOnly,
    /// 평문으로 붙여넣기(메뉴 항목 — 모드 라벨보다 길다).
    ActionPastePlain,
    /// 고정 토글.
    ActionPin,
    /// 항목 삭제.
    ActionDelete,
    /// 전체 비우기.
    ActionClearAll,
    /// 설정 열기.
    ActionSettings,
    /// 정보.
    ActionAbout,
    /// 종료.
    ActionQuit,
    /// 메인창 열기.
    ActionOpenMain,
    /// 캡처 일시정지 토글.
    ActionPauseCapture,

    // ── 상태 ──────────────────────────────────
    /// 기록이 암호화되어 있음.
    StatusEncrypted,
    /// 로컬 전용(네트워크로 나가지 않음).
    StatusLocalOnly,
    /// 현재 클립보드 내용(트레이 메뉴 머리).
    StatusCurrentClipboard,
    /// 이 환경에서는 클립보드 수집이 불가능함(Wayland/GNOME 등).
    StatusWatchUnsupported,

    // ── 동기화(기본 기능 — 사용자 확정 08-26) ──
    /// 기기 간 동기화.
    SyncTitle,
    /// 동기화 사용 여부.
    SyncEnabled,
    /// 연결된 기기 목록.
    SyncDevices,
    /// 새 기기 승인 대기.
    SyncPendingApproval,
    /// 전송 구간은 종단 암호화된다.
    SyncEndToEnd,

    // ── 설정 화면(registry) — docs/14 ──
    /// 작게.
    SizeSmall,
    /// 보통(기본).
    SizeNormal,
    /// 크게.
    SizeLarge,
    /// 아주 크게.
    SizeExtraLarge,
    /// 가장 크게.
    SizeXLarge,
    /// 분 단위 입력 범위 오류.
    ValMinutesRange,
    /// 콤보의 직접 입력 항목.
    CustomInput,
    /// 목록 컨트롤 빈 상태 표시(ListEditor · 09-01).
    ListEmpty,
    /// UI 글꼴 설정(09-01 사용자 요청 — JetBrains Mono 등 시스템 글꼴 지정).
    SetUiFont,
    SetUiFontDesc,
    /// ★ 고정폭 글꼴(09-04 — 터미널/코드 리치 런).
    SetMonoFont,
    SetMonoFontDesc,
    /// 보관 기간(T-13 · 09-01).
    SetMaxAge,
    SetMaxAgeDesc,
    /// 총용량 상한(T-13 · 09-01).
    SetMaxTotal,
    SetMaxTotalDesc,
    /// 시스템 기본 글꼴.
    SystemDefaultFont,
    /// 설정 카테고리: 일반.
    CatGeneral,
    /// 설정 카테고리: 단축키.
    CatShortcuts,
    /// ★ 단축키 설정(09-04): 동작 라벨 3 · 공통 설명 · 캡처 오버레이 문구.
    SetHotkeyOpen,
    SetHotkeyOpenAlt,
    SetHotkeyPastePlain,
    SetHotkeyDesc,
    HotkeyNone,
    HotkeyTitle,
    HotkeyPrompt,
    HotkeyNeedMod,
    HotkeyRemove,
    HotkeyOk,
    HotkeyCancel,
    /// 설정 카테고리: 캡처.
    CatCapture,
    /// 설정 카테고리: 보관.
    CatStorage,
    /// 설정 카테고리: 보안·개인정보.
    CatPrivacy,
    /// 설정 카테고리: 붙여넣기.
    CatPaste,
    /// 설정 카테고리: 모양.
    CatAppearance,
    /// 설정 카테고리: 검색.
    CatSearch,
    /// 설정 카테고리: 동기화.
    CatSync,
    /// 설정 카테고리: 고급.
    CatAdvanced,
    /// 로그인 시 자동 시작.
    SetAutostart,
    /// 자동 시작 설명.
    SetAutostartDesc,
    /// 언어.
    SetLang,
    /// 언어 설명.
    SetLangDesc,
    /// 팝업 위치.
    SetPopupAt,
    /// 팝업 위치 설명(커서 기본).
    SetPopupAtDesc,
    /// 값: 마우스 커서 위치.
    ValCursor,
    /// 값: 화면 중앙.
    ValScreenCenter,
    /// 값: 마지막 위치.
    ValLastPos,
    /// 목록 보기 모드.
    SetViewMode,
    /// 퀵 팝업 보기 모드 설정 라벨(09-02).
    SetPopupView,
    /// 퀵 팝업 보기 모드 설정 설명.
    SetPopupViewDesc,
    /// 보기 모드 설명.
    SetViewModeDesc,
    /// 테마.
    SetTheme,
    /// 테마 설명.
    SetThemeDesc,
    /// 값: 시스템 설정을 따름.
    ValSystem,
    /// 값: 다크.
    ValDark,
    /// 값: 라이트.
    ValLight,
    /// 텍스트 저장.
    SetCapText,
    /// 이미지 저장.
    SetCapImage,
    /// 파일·폴더 저장.
    SetCapFiles,
    /// 서식 저장.
    SetCapRich,
    /// 원본 포맷 보존.
    SetCapNative,
    /// 원본 포맷 설명.
    SetCapNativeDesc,
    /// 최대 항목 수.
    SetMaxItems,
    /// 항목 수 설명.
    SetMaxItemsDesc,
    /// 정렬.
    SetSortBy,
    /// 값: 최근 복사순.
    ValRecentCopy,
    /// 값: 최초 복사순.
    ValFirstCopy,
    /// 값: 복사 횟수순.
    ValCopyCount,
    /// 민감 표식 존중.
    SetRespectMarks,
    /// 표식 설명.
    SetRespectMarksDesc,
    /// 브라우저 암호 관리자 복사 차단(D-79).
    SetConcealBrowserPw,
    /// 위 설명.
    SetConcealBrowserPwDesc,
    /// 차단할 출처 URL 목록(직접 편집).
    SetConcealUrls,
    /// 위 설명.
    SetConcealUrlsDesc,
    /// 제외 앱 목록(FR-S-2).
    SetExcludeApps,
    /// 위 설명.
    SetExcludeAppsDesc,
    /// 트레이 메뉴 — 열기.
    TrayOpen,
    /// 트레이 메뉴 — 종료.
    TrayQuit,
    /// 트레이 메뉴 "설정"(09-01 사용자 요청 — 우클릭에서 바로).
    TraySettings,
    /// ★ Dock 아이콘 표시(T-12e mac · 09-03).
    SetDockIcon,
    StSyncRevoke,
    StSyncDelete,
    StSyncSas,
    SetSyncRetry,
    SetSyncRetryDesc,
    SyncRetryNormal,
    SyncRetryPatient,
    SyncRetryEager,
    SyncRelayOff,
    StSyncLanOnly,
    SyncApprove,
    SyncApproveDesc,
    SyncApproveVerb,
    StSyncApproved,
    StSyncApproveNone,
    StSyncDevApproved,
    StSyncDevNeedsApproval,
    /// 위 설명.
    SetDockIconDesc,
    // ── 메인창·팝업 표시 문자열(09-02 i18n 스윕 — "기본 언어 en인데 한글이 보인다").
    SearchHint,
    MainTitleSuffix,
    MainNoItems,
    MainNoMatch,
    PopupNoItems,
    /// 상태줄 — `{}`에 개수가 들어간다(replacen).
    StatusLine,
    StatusLineFiltered,
    TipPin,
    TipDelete,
    TipCopy,
    TipCopyPlain,
    TipAlwaysTop,
    /// 미리보기 패널 토글 툴팁(09-02 K4).
    TipPreview,
    /// ★ 감시 토글 툴팁(09-04) — 감시 중: 누르면 중지.
    TipCaptureStop,
    /// 감시 중지됨: 누르면 재개.
    TipCaptureResume,
    TipSyncRelay,
    TipSyncLocal,
    TipSyncOff,
    TipSyncDown,
    MenuCopy,
    MenuCopyPlain,
    MenuCopyObject,
    MenuCopyPath,
    MenuPin,
    MenuUnpin,
    MenuEdit,
    MenuDelete,
    /// 컨텍스트 메뉴: 이미지로 복사(렌더 · 09-03).
    MenuCopyImage,
    MenuOrigin,
    DedupLabel,
    /// 팝업 푸터: 스택 순차 붙여넣기 힌트(09-03 ③ — `{}` = 개수).
    HintStack,
    /// 동기화: 핸들(내 아이디) 설정 라벨(09-03 기반).
    SetSyncHandle,
    SetSyncDeviceName,
    SetSyncDeviceNameDesc,
    SetSyncDevices,
    SetSyncDevicesDesc,
    StSyncDevMe,
    StSyncDevOnline,
    StSyncDevAgo,
    /// 동기화: 핸들 설명.
    SetSyncHandleDesc,
    /// 동기화: 페어링 패스프레이즈 라벨.
    SetSyncPass,
    /// 동기화: 패스프레이즈 설명.
    SetSyncPassDesc,
    /// 동기화: 릴레이 서버 주소 라벨.
    SetSyncRelay,
    /// 릴레이 기본 옵션 라벨(공식 서버).
    SyncRelayDefault,
    /// 릴레이 포트 설정 라벨.
    SetSyncPort,
    /// 릴레이 포트 설명.
    SetSyncPortDesc,
    /// 포트 47300 옵션 라벨(실값 표기 — beep 08-22 교훈).
    SyncPort47300,
    /// 연결 테스트 라벨.
    SyncTest,
    /// 연결 테스트 설명.
    SyncTestDesc,
    /// 연결 테스트 버튼 동사.
    SyncTestVerb,
    /// 상태: 접속 중.
    StSyncTesting,
    /// 상태: 접속 성공(`{}` = 서버 · `{}` = 핀 앞 8자).
    StSyncTestOk,
    /// 상태: 접속 실패(`{}` = 사유).
    StSyncTestFail,
    /// 상태: 패스프레이즈 추천됨.
    StSyncPassSuggested,
    /// 연결 해제 라벨.
    SyncDisconnect,
    /// 연결 해제 설명.
    SyncDisconnectDesc,
    /// 연결 해제 버튼 동사.
    SyncDisconnectVerb,
    /// 상태: 해제됨.
    StSyncDisconnected,
    /// ★ 상태: 핸들·암호 미설정(09-04) — 접속 불가.
    StSyncNeedIdentity,
    /// 상태: 연결 안 됨.
    StSyncNotConnected,
    /// 동기화: 릴레이 주소 설명.
    SetSyncRelayDesc,
    EditorHint,
    HintFiles,
    HintRich,
    HintImage,
    HintDefault,
    CtxSelectAll,
    CtxCopy,
    CtxCut,
    CtxPaste,
    /// 편집 시트 줄 바꿈 스위치 라벨(09-02).
    WrapLabel,
    /// 닫을 때 트레이로.
    SetCloseToTray,
    /// 위 설명.
    SetCloseToTrayDesc,
    /// 이미지 미리보기(썸네일).
    SetImagePreview,
    /// 위 설명.
    SetImagePreviewDesc,
    /// 종료 시 기록 비우기.
    SetClearOnQuit,
    /// 자동 붙여넣기.
    SetPasteAuto,
    /// 자동 붙여넣기 설명.
    SetPasteAutoDesc,
    /// 항상 평문으로.
    SetPastePlainDefault,
    /// 검색 방식.
    SetSearchMode,
    /// 값: 정확히 일치.
    ValExact,
    /// 값: 유사 검색.
    ValFuzzy,
    /// 값: 정규식.
    ValRegex,
    /// ★ 검색 방식 설명(09-04).
    SetSearchModeDesc,
    /// 한글 조합 중 검색.
    SetHangulCompose,
    /// 동기화 설명.
    SetSyncEnabledDesc,
    /// 트레이 최근 항목 수.
    SetTrayRecent,
    /// 진단 로그.
    SetDiagLog,
    /// 로그 설명.
    SetDiagLogDesc,
    /// ★ 기록 모두 삭제(09-04 · 고급) — 라벨 · 설명 · 버튼 · 무장 노트 · 완료 노트.
    SetClearHistory,
    SetClearHistoryDesc,
    SetClearHistoryVerb,
    NoteClearArmed,
    NoteClearDone,
    /// 값: 영어.
    ValLangEn,
    /// 값: 한국어.
    ValLangKo,
    /// 값: 중국어(간체).
    ValLangZh,
    /// 값: 일본어.
    ValLangJa,
}

impl Msg {
    /// `[en, ko, zh, ja]` — 한 줄에 모아 누락을 리뷰에서 잡는다.
    #[must_use]
    pub fn row(self) -> [&'static str; 4] {
        match self {
            Msg::AppName => ["Nexa Clip", "Nexa Clip", "Nexa Clip", "Nexa Clip"],
            Msg::SearchPlaceholder => [
                "type to search…",
                "검색어를 입력하세요…",
                "输入以搜索…",
                "検索…",
            ],
            Msg::EmptyHistory => [
                "Nothing copied yet",
                "복사한 항목이 없습니다",
                "还没有复制记录",
                "コピー履歴がありません",
            ],
            Msg::NoMatches => ["No matches", "일치하는 항목 없음", "无匹配项", "一致なし"],

            Msg::KindText => ["Text", "텍스트", "文本", "テキスト"],
            Msg::KindRichText => [
                "Formatted text",
                "서식 있는 텍스트",
                "带格式文本",
                "書式付きテキスト",
            ],
            Msg::KindImage => ["Image", "이미지", "图片", "画像"],
            Msg::KindFiles => ["Files", "파일", "文件", "ファイル"],
            Msg::KindColor => ["Color", "색상", "颜色", "色"],

            Msg::ViewRich => ["Rich", "일반 보기", "标准视图", "リッチ表示"],
            Msg::ViewCompact => ["Compact", "간략 보기", "简洁视图", "簡易表示"],
            Msg::ViewPlain => ["Plain", "한 줄 보기", "单行视图", "1行表示"],

            Msg::ActionCopy => ["Copy", "복사", "复制", "コピー"],
            Msg::KindObject => ["Object", "개체", "对象", "オブジェクト"],
            Msg::ValItemsRange => [
                "Enter 10-100000",
                "10~100000 사이로 입력하세요",
                "请输入 10~100000",
                "10~100000 で入力してください",
            ],
            Msg::ValTrayCountRange => [
                "Enter 3-20",
                "3~20 사이로 입력하세요",
                "请输入 3~20",
                "3~20 で入力してください",
            ],
            Msg::PasteOriginal => ["Original", "원본", "原始格式", "元の形式"],
            Msg::PastePlain => ["Plain text", "평문", "纯文本", "書式なし"],
            Msg::PasteObject => ["As object", "객체로", "作为对象", "オブジェクトとして"],
            Msg::PastePathOnly => ["Path only", "경로만", "仅路径", "パスのみ"],
            Msg::ActionPastePlain => [
                "Paste as plain text",
                "평문으로 붙여넣기",
                "以纯文本粘贴",
                "書式なしで貼り付け",
            ],
            Msg::ActionPin => ["Pin", "고정", "固定", "ピン留め"],
            Msg::ActionDelete => ["Delete", "삭제", "删除", "削除"],
            Msg::ActionClearAll => ["Clear all", "전체 비우기", "全部清除", "すべて消去"],
            Msg::ActionSettings => ["Preferences…", "설정…", "偏好设置…", "設定…"],
            Msg::ActionAbout => ["About", "정보", "关于", "情報"],
            Msg::ActionQuit => ["Quit", "종료", "退出", "終了"],
            Msg::ActionOpenMain => [
                "Open main window…",
                "메인창 열기…",
                "打开主窗口…",
                "メインウィンドウ…",
            ],
            Msg::ActionPauseCapture => [
                "Pause capturing",
                "캡처 일시정지",
                "暂停采集",
                "取り込みを一時停止",
            ],

            Msg::StatusEncrypted => ["Encrypted", "암호화됨", "已加密", "暗号化済み"],
            Msg::StatusLocalOnly => ["Local only", "로컬 전용", "仅本地", "ローカルのみ"],
            Msg::StatusCurrentClipboard => {
                ["Clipboard", "현재 클립보드", "剪贴板", "クリップボード"]
            }
            Msg::StatusWatchUnsupported => [
                "Clipboard capture is not available in this environment",
                "이 환경에서는 클립보드 수집을 할 수 없습니다",
                "当前环境无法采集剪贴板",
                "この環境ではクリップボードを取得できません",
            ],

            Msg::SyncTitle => ["Device sync", "기기 간 동기화", "设备同步", "デバイス同期"],
            Msg::SyncEnabled => ["Enable sync", "동기화 사용", "启用同步", "同期を有効にする"],
            Msg::SyncDevices => [
                "Linked devices",
                "연결된 기기",
                "已连接设备",
                "接続済みデバイス",
            ],
            Msg::SyncPendingApproval => [
                "Waiting for approval",
                "승인 대기 중",
                "等待批准",
                "承認待ち",
            ],
            Msg::SyncEndToEnd => [
                "End-to-end encrypted in transit",
                "전송 구간 종단 암호화",
                "传输过程端到端加密",
                "転送区間はエンドツーエンド暗号化",
            ],
            Msg::SizeSmall => ["Small", "작게", "小", "小"],
            Msg::SizeNormal => ["Normal", "보통", "中", "標準"],
            Msg::SizeLarge => ["Large", "크게", "大", "大"],
            Msg::SizeExtraLarge => ["Extra large", "아주 크게", "特大", "特大"],
            Msg::SizeXLarge => ["XL", "가장 크게", "超大", "最大"],
            Msg::ValMinutesRange => [
                "Enter minutes in range",
                "분 단위로 범위 안에서 입력하세요",
                "请输入范围内的分钟数",
                "範囲内の分数を入力してください",
            ],
            Msg::CustomInput => ["Custom…", "직접 입력…", "自定义…", "カスタム…"],
            Msg::ListEmpty => ["No items", "내용 없음", "无项目", "項目なし"],
            Msg::SetUiFont => ["UI font", "UI 글꼴", "UI 字体", "UI フォント"],
            Msg::SetUiFontDesc => [
                "Font family name (e.g. JetBrains Mono). Empty = system default. Applies after restart",
                "글꼴 이름(예: JetBrains Mono) · 비우면 시스템 기본 · 재시작 후 적용",
                "字体名称（例：JetBrains Mono）。空 = 系统默认。重启后生效",
                "フォント名（例: JetBrains Mono）。空欄 = システム既定。再起動後に適用",
            ],
            Msg::SetMonoFont => ["Monospace font", "고정폭 글꼴", "等宽字体", "等幅フォント"],
            Msg::SetMonoFontDesc => [
                "Used for terminal/code copies (colors kept). Empty = Nerd Font → D2Coding → Consolas… first installed. Applies after restart",
                "터미널·코드 복사본(색 유지)에 쓰는 글꼴 · 비우면 Nerd Font → D2Coding → Consolas… 설치된 첫 후보 · 재시작 후 적용",
                "用于终端/代码副本（保留颜色）。空 = Nerd Font → D2Coding → Consolas… 首个已安装。重启后生效",
                "ターミナル/コードのコピー（色保持）に使用。空欄 = Nerd Font → D2Coding → Consolas… 最初の導入済み。再起動後に適用",
            ],
            Msg::SystemDefaultFont => ["System default", "시스템 기본", "系统默认", "システム標準"],
            Msg::CatGeneral => ["General", "일반", "常规", "一般"],
            Msg::CatShortcuts => ["Shortcuts", "단축키", "快捷键", "ショートカット"],
            Msg::SetHotkeyOpen => ["Show quick popup", "퀵 팝업 띄우기", "显示快捷弹窗", "クイック ポップアップを表示"],
            Msg::SetHotkeyOpenAlt => [
                "Show quick popup (secondary)",
                "퀵 팝업 띄우기 (보조)",
                "显示快捷弹窗（备用）",
                "クイック ポップアップを表示（予備）",
            ],
            Msg::SetHotkeyPastePlain => [
                "Paste clipboard as plain text",
                "클립보드를 평문으로 붙여넣기",
                "以纯文本粘贴剪贴板",
                "クリップボードをプレーンテキストで貼り付け",
            ],
            Msg::SetHotkeyDesc => [
                "Global — works from any app. Click the shortcut to change it. Windows applies immediately; macOS/Linux after restart",
                "전역 — 어느 앱에서나 동작합니다. 조합을 누르면 바꿉니다. Windows는 즉시, macOS/Linux는 재시작 후 적용",
                "全局——在任何应用中生效。点击组合键以更改。Windows 立即生效；macOS/Linux 重启后生效",
                "グローバル — どのアプリでも動作。組み合わせを押すと変更。Windows は即時、macOS/Linux は再起動後に適用",
            ],
            Msg::HotkeyNone => ["None", "없음", "无", "なし"],
            Msg::HotkeyTitle => ["Set shortcut", "단축키 지정", "设置快捷键", "ショートカットを設定"],
            Msg::HotkeyPrompt => [
                "Press the key combination",
                "키 조합을 누르세요",
                "请按下组合键",
                "キーの組み合わせを押してください",
            ],
            Msg::HotkeyNeedMod => [
                "Include Ctrl, Shift, Alt or Win (F-keys may stand alone)",
                "Ctrl·Shift·Alt·Win 중 하나를 포함하세요 (F키는 단독 가능)",
                "请包含 Ctrl、Shift、Alt 或 Win（F 键可单独使用）",
                "Ctrl・Shift・Alt・Win のいずれかを含めてください（F キーは単独可）",
            ],
            Msg::HotkeyRemove => ["Remove shortcut", "바로가기 제거", "移除快捷键", "ショートカットを削除"],
            Msg::HotkeyOk => ["OK", "확인", "确定", "OK"],
            Msg::HotkeyCancel => ["Cancel", "취소", "取消", "キャンセル"],
            Msg::CatCapture => ["Capture", "캡처", "采集", "取り込み"],
            Msg::CatStorage => ["Storage", "보관", "存储", "保存"],
            Msg::CatPrivacy => ["Privacy", "보안·개인정보", "隐私与安全", "プライバシー"],
            Msg::CatPaste => ["Paste", "붙여넣기", "粘贴", "貼り付け"],
            Msg::CatAppearance => ["Appearance", "모양", "外观", "外観"],
            Msg::CatSearch => ["Search", "검색", "搜索", "検索"],
            Msg::CatSync => ["Sync", "동기화", "同步", "同期"],
            Msg::CatAdvanced => ["Advanced", "고급", "高级", "詳細"],
            Msg::SetAutostart => [
                "Launch at login",
                "로그인 시 자동 시작",
                "登录时启动",
                "ログイン時に起動",
            ],
            Msg::SetAutostartDesc => [
                "Start Nexa Clip when you log in",
                "로그인하면 Nexa Clip을 자동으로 시작합니다",
                "登录后自动启动 Nexa Clip",
                "ログイン時に自動的に起動します",
            ],
            Msg::SetLang => ["Language", "언어", "语言", "言語"],
            Msg::SetLangDesc => [
                "Interface language",
                "화면에 쓰이는 언어",
                "界面语言",
                "表示言語",
            ],
            Msg::SetPopupAt => ["Popup at", "팝업 위치", "弹出位置", "ポップアップ位置"],
            Msg::SetPopupAtDesc => [
                "Where the quick popup appears",
                "퀵 팝업이 뜨는 위치입니다",
                "快捷弹窗出现的位置",
                "クイックポップアップが開く位置",
            ],
            Msg::ValCursor => ["At cursor", "마우스 위치", "鼠标位置", "カーソル位置"],
            Msg::ValScreenCenter => ["Screen center", "화면 중앙", "屏幕中央", "画面中央"],
            Msg::ValLastPos => ["Last position", "마지막 위치", "上次位置", "前回の位置"],
            Msg::SetViewMode => ["List view", "목록 보기", "列表视图", "リスト表示"],
            Msg::SetPopupView => [
                "Popup view",
                "팝업 보기",
                "弹窗视图",
                "ポップアップ表示",
            ],
            Msg::SetPopupViewDesc => [
                "View mode for the quick popup (default: rich)",
                "퀵 팝업의 보기 모드 (기본: 리치)",
                "快捷弹窗的视图模式（默认：丰富）",
                "クイックポップアップの表示モード（既定: リッチ）",
            ],
            Msg::SetViewModeDesc => [
                "How much of each item to show",
                "항목을 얼마나 펼쳐 보일지",
                "每项显示多少内容",
                "各項目をどこまで表示するか",
            ],
            Msg::SetTheme => ["Theme", "테마", "主题", "テーマ"],
            Msg::SetThemeDesc => [
                "Follow the system or pick one",
                "시스템을 따르거나 직접 고릅니다",
                "跟随系统或手动选择",
                "システムに従うか手動で選択",
            ],
            Msg::ValSystem => ["System", "시스템", "跟随系统", "システム"],
            Msg::ValDark => ["Dark", "다크", "深色", "ダーク"],
            Msg::ValLight => ["Light", "라이트", "浅色", "ライト"],
            Msg::SetCapText => ["Save text", "텍스트 저장", "保存文本", "テキストを保存"],
            Msg::SetCapImage => ["Save images", "이미지 저장", "保存图片", "画像を保存"],
            Msg::SetCapFiles => ["Save files", "파일 저장", "保存文件", "ファイルを保存"],
            Msg::SetCapRich => ["Save formatting", "서식 저장", "保存格式", "書式を保存"],
            Msg::SetCapNative => [
                "Keep original formats",
                "원본 포맷 보존",
                "保留原始格式",
                "元の形式を保持",
            ],
            Msg::SetCapNativeDesc => [
                "Keeps Word/PowerPoint objects pasteable as objects",
                "Word·PPT 개체를 개체 그대로 붙여넣을 수 있게 합니다",
                "让 Word/PPT 对象仍可作为对象粘贴",
                "Word/PPT のオブジェクトをそのまま貼り付けられます",
            ],
            Msg::SetMaxItems => ["Maximum items", "최대 항목 수", "最大条目数", "最大項目数"],
            Msg::SetMaxItemsDesc => [
                "Older items are removed first; pinned items are kept",
                "오래된 항목부터 지워집니다. 고정한 항목은 남습니다",
                "优先删除旧条目，固定项会保留",
                "古い項目から削除されます。ピン留めは残ります",
            ],
            Msg::SetSortBy => ["Sort by", "정렬", "排序方式", "並び順"],
            Msg::ValRecentCopy => [
                "Time of last copy",
                "최근 복사순",
                "最近复制时间",
                "最終コピー順",
            ],
            Msg::ValFirstCopy => [
                "Time of first copy",
                "최초 복사순",
                "首次复制时间",
                "初回コピー順",
            ],
            Msg::ValCopyCount => [
                "Number of copies",
                "복사 횟수순",
                "复制次数",
                "コピー回数順",
            ],
            Msg::SetRespectMarks => [
                "Respect password manager marks",
                "비밀번호 관리자 표식 존중",
                "遵循密码管理器标记",
                "パスワード管理アプリの印を尊重",
            ],
            Msg::SetRespectMarksDesc => [
                "Recommended. Skips anything marked as secret",
                "권장. 비밀로 표시된 것은 저장하지 않습니다",
                "推荐。跳过标记为机密的内容",
                "推奨。機密と印が付いたものは保存しません",
            ],
            Msg::SetConcealBrowserPw => [
                "Block browser password copies",
                "브라우저 암호 관리자 복사 차단",
                "阻止浏览器密码管理器复制",
                "ブラウザのパスワードコピーをブロック",
            ],
            Msg::SetConcealBrowserPwDesc => [
                "Edge/Chrome password managers don't mark copies as secret. Detects them by source page",
                "Edge/Chrome 암호 관리자는 비밀 표식을 붙이지 않습니다. 복사 출처 페이지로 알아냅니다",
                "Edge/Chrome 密码管理器不会标记机密。按复制来源页面识别",
                "Edge/Chrome のパスワード管理は機密印を付けません。コピー元ページで検出します",
            ],
            Msg::SetConcealUrls => [
                "Blocked source pages",
                "차단할 출처 페이지",
                "屏蔽的来源页面",
                "ブロックするコピー元ページ",
            ],
            Msg::SetConcealUrlsDesc => [
                "URL prefixes separated by ; — copies from these pages are not recorded. Empty restores defaults",
                "; 로 구분한 URL 접두 — 이 페이지에서 복사한 것은 기록하지 않습니다. 비우면 기본 목록으로 돌아갑니다",
                "以 ; 分隔的 URL 前缀 — 来自这些页面的复制不记录。清空则恢复默认",
                "; 区切りの URL 接頭辞 — これらのページからのコピーは記録しません。空にすると既定に戻ります",
            ],
            Msg::SetExcludeApps => [
                "Excluded apps",
                "제외할 앱",
                "排除的应用",
                "除外するアプリ",
            ],
            Msg::SetExcludeAppsDesc => [
                "App names separated by ; (e.g. KeePass) — copies from these apps are never recorded",
                "; 로 구분한 앱 이름(예: KeePass) — 이 앱에서 복사한 것은 기록하지 않습니다",
                "以 ; 分隔的应用名（如 KeePass）— 来自这些应用的复制不记录",
                "; 区切りのアプリ名（例: KeePass）— これらのアプリからのコピーは記録しません",
            ],
            Msg::SetMaxAge => ["Keep for (days)", "보관 기간(일)", "保留天数", "保持日数"],
            Msg::SetMaxAgeDesc => [
                "0 = keep forever. Older unpinned items are removed",
                "0 = 무제한 · 기한이 지난 비고정 항목을 삭제합니다",
                "0 = 永久保留。删除超期的未固定项",
                "0 = 無期限。期限切れの未固定項目を削除",
            ],
            Msg::SetMaxTotal => ["Storage limit (MB)", "총용량 상한(MB)", "存储上限(MB)", "保存上限(MB)"],
            Msg::SetMaxTotalDesc => [
                "Oldest unpinned items are removed first when over the limit",
                "초과하면 오래된 비고정 항목부터 삭제합니다(핀 면제)",
                "超出时从最旧的未固定项开始删除",
                "超過時は古い未固定項目から削除",
            ],
            Msg::TrayOpen => ["Open", "열기", "打开", "開く"],
            Msg::TrayQuit => ["Quit", "종료", "退出", "終了"],
            Msg::TraySettings => ["Settings", "설정", "设置", "設定"],
            Msg::SetDockIcon => [
                "Show Dock icon",
                "Dock 아이콘 표시",
                "显示 Dock 图标",
                "Dock アイコンを表示",
            ],
            Msg::SyncApprove => ["Approve devices", "기기 승인", "批准设备", "端末を承認"],
            Msg::SyncApproveDesc => [
                "Approve each device on its row (the same 6-digit code must show on both devices). Clipboard is shared only with approved devices.",
                "각 기기 행에서 승인합니다(같은 6자리 대조 코드가 양쪽에 떠야 합니다). 클립보드는 승인된 기기와만 오갑니다.",
                "在各设备行上批准（两台设备须显示相同的 6 位校验码）。剪贴板仅与已批准的设备共享。",
                "各端末の行で承認します（同じ6桁の照合コードが両端末に出ること）。クリップボードは承認済み端末とのみ共有します。",
            ],
            Msg::SyncApproveVerb => ["Approve", "승인", "批准", "承認"],
            Msg::StSyncApproved => [
                "Approved {} device(s) — clipboard now syncs",
                "{}대 승인 — 이제 클립보드가 동기화됩니다",
                "已批准 {} 台设备——剪贴板现在会同步",
                "{} 台を承認 — クリップボードが同期されます",
            ],
            Msg::StSyncApproveNone => [
                "No connected device to approve",
                "승인할 연결된 기기가 없습니다",
                "没有可批准的已连接设备",
                "承認できる接続中の端末がありません",
            ],
            Msg::StSyncDevApproved => ["approved", "승인됨", "已批准", "承認済み"],
            Msg::StSyncDevNeedsApproval => ["approval needed", "승인 필요", "需要批准", "要承認"],
            Msg::SetSyncRetry => ["Retry backoff", "재시도 대기", "重试退避", "再試行の待機"],
            Msg::SetSyncRetryDesc => [
                "Wait doubles after each failure (5→10→20 s…, up to 5 min), ±20% random, reset on success. Same rule for relay reconnect, device redial, pairing search and LAN dial. Patient: 15 s→15 min · Eager: 2 s→1 min",
                "실패할 때마다 대기가 2배(5→10→20초…, 최대 5분), ±20% 무작위, 성공하면 초기화. 릴레이 재접속·기기 재다이얼·페어링 탐색·LAN 직결 모두 같은 규칙. 느긋: 15초→15분 · 적극: 2초→1분",
                "每次失败后等待翻倍（5→10→20 秒…，最长 5 分钟），±20% 随机，成功后重置。中继重连、设备重拨、配对搜索、局域网直连同一规则。耐心：15 秒→15 分钟 · 积极：2 秒→1 分钟",
                "失敗ごとに待機が2倍（5→10→20秒…、最大5分）、±20%ランダム、成功で初期化。リレー再接続・端末再ダイヤル・ペアリング探索・LAN直結すべて同じ規則。のんびり: 15秒→15分 · 積極: 2秒→1分",
            ],
            Msg::SyncRetryNormal => ["Normal (5 s → 5 min)", "표준 (5초 → 5분)", "标准（5 秒 → 5 分钟）", "標準（5秒 → 5分）"],
            Msg::SyncRetryPatient => ["Patient (15 s → 15 min)", "느긋 (15초 → 15분)", "耐心（15 秒 → 15 分钟）", "のんびり（15秒 → 15分）"],
            Msg::SyncRetryEager => ["Eager (2 s → 1 min)", "적극 (2초 → 1분)", "积极（2 秒 → 1 分钟）", "積極（2秒 → 1分）"],
            Msg::SyncRelayOff => ["None", "None", "None", "None"],
            Msg::StSyncLanOnly => [
                "Relay: None — devices on the same network connect directly",
                "릴레이 None — 같은 네트워크의 기기끼리 직접 연결됩니다",
                "中继已关闭——同一网络的设备直接连接",
                "リレー未使用 — 同じネットワークの端末同士が直接つながります",
            ],
            Msg::StSyncRevoke => ["Revoke", "해제", "撤销", "解除"],
            Msg::StSyncDelete => ["Remove", "삭제", "删除", "削除"],
            Msg::StSyncSas => ["code {}", "대조 {}", "校验 {}", "照合 {}"],
            Msg::SetDockIconDesc => [
                "macOS only: off hides the app from Dock and Cmd+Tab — open it from the menu bar icon",
                "macOS 전용: 끄면 Dock·Cmd+Tab에서 사라지고 메뉴 막대 아이콘에서만 엽니다",
                "仅 macOS：关闭后从 Dock 和 Cmd+Tab 消失，仅可从菜单栏图标打开",
                "macOS のみ: オフにすると Dock・Cmd+Tab から消え、メニューバーのアイコンからのみ開けます",
            ],
            Msg::SearchHint => ["Search…", "검색…", "搜索…", "検索…"],
            Msg::MainTitleSuffix => [
                "Clipboard Manager",
                "클립보드 관리자",
                "剪贴板管理器",
                "クリップボードマネージャー",
            ],
            Msg::MainNoItems => [
                "No items yet — copy something",
                "항목이 없습니다 — 복사하면 여기 쌓입니다",
                "暂无项目 — 复制即可收集",
                "項目なし — コピーすると溜まります",
            ],
            Msg::MainNoMatch => ["No matches", "일치하는 항목이 없습니다", "无匹配项", "一致なし"],
            Msg::PopupNoItems => [
                "Nothing captured yet — try copying",
                "아직 잡은 항목이 없습니다 — 복사해 보세요",
                "尚未捕获 — 试试复制",
                "まだ何もありません — コピーしてみて",
            ],
            Msg::StatusLine => ["{} items · encrypted · local", "{}개 · 암호화 · 로컬", "{} 项 · 已加密 · 本地", "{} 件 · 暗号化 · ローカル"],
            Msg::StatusLineFiltered => ["{} / {} items · encrypted · local", "{} / {}개 · 암호화 · 로컬", "{} / {} 项 · 已加密 · 本地", "{} / {} 件 · 暗号化 · ローカル"],
            Msg::TipPin => ["Pin/unpin (Ctrl+P)", "고정/해제 (Ctrl+P)", "固定/取消 (Ctrl+P)", "固定/解除 (Ctrl+P)"],
            Msg::TipDelete => ["Delete (Del)", "삭제 (Delete)", "删除 (Del)", "削除 (Del)"],
            Msg::TipCopy => ["Copy (Enter)", "복사 (Enter)", "复制 (Enter)", "コピー (Enter)"],
            Msg::TipCopyPlain => [
                "Copy as plain text (Shift+Enter)",
                "평문으로 복사 (Shift+Enter)",
                "复制为纯文本 (Shift+Enter)",
                "プレーンでコピー (Shift+Enter)",
            ],
            Msg::TipAlwaysTop => ["Always on top", "최상위 고정", "总在最前", "常に手前"],
            Msg::TipPreview => ["Preview", "미리보기", "预览", "プレビュー"],
            Msg::TipCaptureStop => [
                "Capturing — click to stop",
                "캡처 중 — 누르면 캡처 중지",
                "采集中 — 点击停止",
                "取り込み中 — 押すと停止",
            ],
            Msg::TipCaptureResume => [
                "Capture stopped — click to resume",
                "캡처 중지됨 — 누르면 캡처 재개",
                "已停止采集 — 点击恢复",
                "取り込み停止中 — 押すと再開",
            ],
            Msg::TipSyncRelay => [
                "Green: relay connected — syncing over the internet and the same network",
                "녹색: 릴레이 서버 연결됨 — 인터넷·같은 네트워크 모두 동기화",
                "绿色：已连接中继——通过互联网和同一网络同步",
                "緑: リレー接続中 — インターネット・同一ネットワークで同期",
            ],
            Msg::TipSyncLocal => [
                "Blue: relay None — syncing only on the same network",
                "파랑: 릴레이 None — 같은 네트워크에서만 동기화",
                "蓝色：中继为 None——仅在同一网络同步",
                "青: リレー None — 同じネットワーク内のみ同期",
            ],
            Msg::TipSyncOff => [
                "Gray: sync is off",
                "회색: 동기화 사용 안 함",
                "灰色：同步已关闭",
                "灰: 同期オフ",
            ],
            Msg::TipSyncDown => [
                "Dim: relay set but not connected (connecting / failed / disconnected)",
                "흐림: 릴레이 설정됨, 미연결(접속 중 · 실패 · 해제)",
                "暗淡：已设置中继但未连接（连接中 / 失败 / 已断开）",
                "淡色: リレー設定済みだが未接続（接続中・失敗・切断）",
            ],
            Msg::MenuCopy => ["Copy", "복사", "复制", "コピー"],
            Msg::MenuCopyPlain => ["Copy as plain text", "평문으로 복사", "复制为纯文本", "プレーンでコピー"],
            Msg::MenuCopyObject => ["Copy as object", "개체로 복사", "复制为对象", "オブジェクトでコピー"],
            Msg::MenuCopyPath => ["Copy paths only", "경로만 복사", "仅复制路径", "パスのみコピー"],
            Msg::MenuPin => ["Pin", "고정", "固定", "固定"],
            Msg::MenuUnpin => ["Unpin", "고정 해제", "取消固定", "固定解除"],
            Msg::MenuEdit => ["Edit (as plain text)", "편집(평문화)", "编辑(纯文本)", "編集(プレーン化)"],
            Msg::MenuDelete => ["Delete", "삭제", "删除", "削除"],
            Msg::MenuOrigin => ["From {}", "출처 {}", "来自 {}", "送信元 {}"],
            Msg::DedupLabel => ["Hide duplicates", "중복 제외", "隐藏重复", "重複を隠す"],
            Msg::MenuCopyImage => [
                "Copy as image",
                "이미지로 복사",
                "复制为图片",
                "画像としてコピー",
            ],
            Msg::SetSyncHandle => ["Handle", "내 아이디(핸들)", "我的 ID", "ハンドル"],
            Msg::SetSyncHandleDesc => [
                "Short public name shared by your devices (e.g. kiros33)",
                "내 기기들이 공유하는 짧은 공개 이름 (예: kiros33)",
                "您的设备共享的简短公开名称（例如 kiros33）",
                "端末間で共有する短い公開名（例: kiros33）",
            ],
            Msg::SetSyncDeviceName => ["Device name", "기기 이름", "设备名称", "デバイス名"],
            Msg::SetSyncDeviceNameDesc => [
                "Shown to your other devices to tell them apart (blank = computer name)",
                "다른 기기에 보이는 이 기기의 이름 — 같은 아이디의 기기를 구별합니다 (비우면 컴퓨터 이름)",
                "显示给其他设备以区分本机（留空 = 计算机名）",
                "他の端末に表示されるこの端末の名前（空欄 = コンピューター名）",
            ],
            Msg::SetSyncDevices => ["Devices", "기기 목록", "设备", "端末一覧"],
            Msg::SetSyncDevicesDesc => [
                "No devices met yet — enter the same handle and passphrase on another device, then Test. Approve each device on its row once the same 6-digit code shows on both.",
                "아직 만난 기기가 없습니다 — 다른 기기에 같은 아이디·페어링 암호를 넣고 Test",
                "尚未遇到设备——在另一台设备输入相同的账号和配对口令后按 Test",
                "まだ端末に出会っていません — 別の端末に同じ ID とパスフレーズを入れて Test",
            ],
            Msg::StSyncDevMe => ["This device", "이 기기", "本机", "この端末"],
            Msg::StSyncDevOnline => ["connected", "연결됨", "已连接", "接続中"],
            Msg::StSyncDevAgo => ["last seen {} ago", "마지막 접속 {} 전", "上次 {} 前", "最終 {} 前"],
            Msg::SetSyncPass => ["Pairing passphrase", "페어링 암호", "配对口令", "ペアリングパスフレーズ"],
            Msg::SetSyncPassDesc => [
                "Secret used only to derive the meeting point — never sent to the server",
                "만남 지점 파생에만 쓰는 비밀 — 서버로 전송되지 않습니다",
                "仅用于派生会合点的秘密——不会发送到服务器",
                "待ち合わせ地点の導出のみに使う秘密 — サーバーへは送られません",
            ],
            Msg::SetSyncRelay => ["Relay server", "릴레이 서버", "中继服务器", "リレーサーバー"],
            Msg::SyncRelayDefault => [
                "beepd.sosomlab.com",
                "beepd.sosomlab.com",
                "beepd.sosomlab.com",
                "beepd.sosomlab.com",
            ],
            Msg::SetSyncPort => ["Relay port", "릴레이 포트", "中继端口", "リレーポート"],
            Msg::SetSyncPortDesc => [
                "TCP control port of the relay. 47300 is the default port of the official relay.",
                "릴레이의 TCP 제어 포트. 47300이 공식 릴레이의 기본 포트입니다.",
                "中继的 TCP 控制端口。47300 为官方中继的默认端口。",
                "リレーの TCP 制御ポート。47300 は公式リレーの既定ポートです。",
            ],
            Msg::SyncPort47300 => [
                "47300",
                "47300",
                "47300",
                "47300",
            ],
            Msg::SyncTest => ["Connection test", "연결 테스트", "连接测试", "接続テスト"],
            Msg::SyncTestDesc => [
                "Try connecting to the relay with the current settings",
                "지금 설정값으로 릴레이에 실제 접속해 봅니다",
                "使用当前设置实际连接中继",
                "現在の設定でリレーへ実際に接続します",
            ],
            Msg::SyncTestVerb => ["Test", "테스트", "测试", "テスト"],
            Msg::StSyncTesting => [
                "Connecting…",
                "접속 중…",
                "连接中…",
                "接続中…",
            ],
            Msg::StSyncTestOk => [
                "Connected — {} · server pin {}",
                "접속 성공 — {} · 서버 핀 {}",
                "连接成功 — {} · 服务器指纹 {}",
                "接続成功 — {} · サーバーピン {}",
            ],
            Msg::StSyncTestFail => [
                "Failed — {}",
                "실패 — {}",
                "失败 — {}",
                "失敗 — {}",
            ],
            Msg::SyncDisconnect => ["Disconnect", "연결 해제", "断开连接", "切断"],
            Msg::SyncDisconnectDesc => [
                "Drop the current relay session (reconnects on next start)",
                "지금 릴레이 세션을 끊습니다 (다음 시작 때 다시 연결)",
                "断开当前中继会话（下次启动时重新连接）",
                "現在のリレーセッションを切断（次回起動時に再接続）",
            ],
            Msg::SyncDisconnectVerb => ["Disconnect", "해제", "断开", "切断"],
            Msg::StSyncNeedIdentity => [
                "Handle and pairing passphrase are both required — nothing connects until both are set",
                "핸들과 페어링 암호가 모두 있어야 합니다 — 둘 다 채우기 전엔 접속하지 않습니다",
                "需要同时填写 Handle 和配对口令——否则不会连接",
                "ハンドルとペアリング パスフレーズの両方が必要です — 揃うまで接続しません",
            ],
            Msg::StSyncDisconnected => [
                "Disconnected — reconnect with Test or on next start",
                "해제됨 — Test 또는 다음 시작 때 다시 연결합니다",
                "已断开——按 Test 或下次启动时重新连接",
                "切断しました — Test か次回起動時に再接続します",
            ],
            Msg::StSyncNotConnected => [
                "Not connected",
                "연결되어 있지 않습니다",
                "未连接",
                "接続されていません",
            ],
            Msg::StSyncPassSuggested => [
                "Suggested a passphrase — press the eye to view",
                "패스프레이즈를 추천해 채웠습니다 — 눈 버튼으로 확인",
                "已生成推荐口令——点眼睛图标查看",
                "パスフレーズを提案しました — 目のボタンで確認",
            ],
            Msg::SetSyncRelayDesc => [
                "Relay address — beepd.sosomlab.com is the official SosomLab relay (default). None = same network only (no server; port, Test and Disconnect are not used).",
                "릴레이 주소 — beepd.sosomlab.com은 SosomLab 공식 릴레이(기본값). None = 같은 네트워크만(서버 없음 · 포트·Test·Disconnect 사용 안 함).",
                "中继地址 — beepd.sosomlab.com 为 SosomLab 官方中继（默认）。None = 仅同一网络（无服务器；端口、Test、Disconnect 不使用）。",
                "リレーアドレス — beepd.sosomlab.com は SosomLab 公式リレー（既定）。None = 同じネットワークのみ（サーバーなし・ポート・Test・Disconnect は使いません）。",
            ],
            Msg::HintStack => [
                "Enter pastes {} in order · Ctrl+Click/Ctrl+Space toggles · Esc",
                "Enter = {}개 순차 붙여넣기 · Ctrl+클릭/Ctrl+Space 선택 · Esc",
                "Enter 按序粘贴 {} 项 · Ctrl+点击/Ctrl+Space 选择 · Esc",
                "Enter で {} 件を順に貼り付け · Ctrl+クリック/Ctrl+Space 選択 · Esc",
            ],
            Msg::EditorHint => [
                "Ctrl+Enter save · Esc cancel · Alt+Z wrap (saved as plain text)",
                "Ctrl+Enter 저장 · Esc 취소 · Alt+Z 줄 바꿈 (평문으로 저장됩니다)",
                "Ctrl+Enter 保存 · Esc 取消 · Alt+Z 换行 (保存为纯文本)",
                "Ctrl+Enter 保存 · Esc 取消 · Alt+Z 折り返し (プレーンで保存)",
            ],
            Msg::HintFiles => [
                "Enter original · Ctrl object · Alt paths · Shift plain · Esc close",
                "Enter 원본 · Ctrl 개체 · Alt 경로 · ⇧ 평문 · Esc 닫기",
                "Enter 原始 · Ctrl 对象 · Alt 路径 · Shift 纯文本 · Esc 关闭",
                "Enter 原本 · Ctrl オブジェクト · Alt パス · Shift プレーン · Esc 閉じる",
            ],
            Msg::HintRich => [
                "Enter original · Shift+Enter plain · Esc close",
                "Enter 원본 · ⇧Enter 평문 · Esc 닫기",
                "Enter 原始 · Shift+Enter 纯文本 · Esc 关闭",
                "Enter 原本 · Shift+Enter プレーン · Esc 閉じる",
            ],
            Msg::HintImage => ["Enter original · Esc close", "Enter 원본 · Esc 닫기", "Enter 原始 · Esc 关闭", "Enter 原本 · Esc 閉じる"],
            Msg::HintDefault => ["Enter paste · Esc close", "Enter 붙여넣기 · Esc 닫기", "Enter 粘贴 · Esc 关闭", "Enter 貼り付け · Esc 閉じる"],
            Msg::CtxSelectAll => ["Select All", "전체 선택", "全选", "すべて選択"],
            Msg::CtxCopy => ["Copy", "복사", "复制", "コピー"],
            Msg::CtxCut => ["Cut", "잘라내기", "剪切", "切り取り"],
            Msg::CtxPaste => ["Paste", "붙여넣기", "粘贴", "貼り付け"],
            Msg::WrapLabel => ["Wrap", "줄 바꿈", "换行", "折り返し"],
            Msg::SetCloseToTray => [
                "Keep running in tray when closed",
                "창을 닫아도 트레이에 남기",
                "关闭窗口后保留在托盘",
                "閉じてもトレイに常駐",
            ],
            Msg::SetCloseToTrayDesc => [
                "Closing the window hides it to the tray instead of quitting",
                "창을 닫으면 종료 대신 트레이로 숨습니다",
                "关闭窗口时隐藏到托盘而不是退出",
                "ウィンドウを閉じると終了せずトレイに隠れます",
            ],
            Msg::SetImagePreview => [
                "Image previews",
                "이미지 미리보기",
                "图片预览",
                "画像プレビュー",
            ],
            Msg::SetImagePreviewDesc => [
                "Show thumbnails for image items in the list. Off shows size only",
                "목록의 이미지 항목에 썸네일을 보여 줍니다. 끄면 크기만 표시합니다",
                "在列表中为图片项显示缩略图。关闭则只显示尺寸",
                "リストの画像項目にサムネイルを表示します。オフはサイズのみ",
            ],
            Msg::SetClearOnQuit => [
                "Clear history on quit",
                "종료 시 기록 비우기",
                "退出时清空历史",
                "終了時に履歴を消去",
            ],
            Msg::SetPasteAuto => [
                "Paste automatically",
                "자동 붙여넣기",
                "自动粘贴",
                "自動で貼り付け",
            ],
            Msg::SetPasteAutoDesc => [
                "Selecting an item pastes it into the window you came from",
                "항목을 고르면 원래 있던 창에 바로 붙여넣습니다",
                "选择条目后直接粘贴到原窗口",
                "項目を選ぶと元のウィンドウに貼り付けます",
            ],
            Msg::SetPastePlainDefault => [
                "Always paste as plain text",
                "항상 평문으로 붙여넣기",
                "始终以纯文本粘贴",
                "常に書式なしで貼り付け",
            ],
            Msg::SetSearchMode => ["Search mode", "검색 방식", "搜索方式", "検索方法"],
            Msg::ValExact => ["Exact", "정확히", "精确", "完全一致"],
            Msg::ValFuzzy => ["Fuzzy", "유사", "模糊", "あいまい"],
            Msg::ValRegex => ["Regular expression", "정규식", "正则表达式", "正規表現"],
            Msg::SetSearchModeDesc => [
                "Searches label and full text, case-insensitive. Exact = substring · Fuzzy = every space-separated word, any order · Regex = built-in engine (. * + ? {n,m} [] () | ^ $ \\d \\w \\s \\b); invalid pattern falls back to Exact",
                "라벨과 본문 전체를 대소문자 무시로 찾습니다. 정확히 = 부분 문자열 · 유사 = 띄어쓴 단어 전부(순서 무관) · 정규식 = 자체 엔진(. * + ? {n,m} [] () | ^ $ \\d \\w \\s \\b) · 잘못된 패턴은 정확히로 대체",
                "搜索标签和全文，忽略大小写。精确 = 子串 · 模糊 = 所有空格分隔的词，顺序不限 · 正则 = 内置引擎 (. * + ? {n,m} [] () | ^ $ \\d \\w \\s \\b)；无效模式回退为精确",
                "ラベルと本文全体を大文字小文字無視で検索。完全 = 部分文字列 · あいまい = 空白区切りの語をすべて（順不同） · 正規表現 = 内蔵エンジン (. * + ? {n,m} [] () | ^ $ \\d \\w \\s \\b)；無効なパターンは完全に戻す",
            ],
            Msg::SetHangulCompose => [
                "Search while composing Hangul",
                "한글 조합 중에도 검색",
                "输入韩文时即时搜索",
                "ハングル入力中も検索",
            ],
            Msg::SetSyncEnabledDesc => [
                "Devices you approved share clipboard items end-to-end encrypted",
                "승인한 기기끼리 종단 암호화로 클립보드를 나눕니다",
                "已批准的设备之间端到端加密共享剪贴板",
                "承認したデバイス間でE2E暗号化して共有",
            ],
            Msg::SetTrayRecent => [
                "Recent items in tray menu",
                "트레이 메뉴의 최근 항목 수",
                "托盘菜单中的最近条目数",
                "トレイメニューの最近の項目数",
            ],
            Msg::SetClearHistory => [
                "Delete all history",
                "기록 모두 삭제",
                "删除全部记录",
                "履歴をすべて削除",
            ],
            Msg::SetClearHistoryDesc => [
                "Removes every clipboard item except pinned ones, from memory and the encrypted store. Click twice within 2 seconds to confirm. Cannot be undone",
                "고정 항목을 뺀 모든 클립보드 기록을 메모리와 암호화 저장소에서 지웁니다. 2초 안에 두 번 눌러야 실행되며 되돌릴 수 없습니다",
                "从内存和加密存储中删除除固定项外的全部剪贴板记录。2 秒内点击两次以确认。无法撤销",
                "固定項目を除くすべての履歴をメモリと暗号化ストアから削除します。2 秒以内に 2 回押して確定。元に戻せません",
            ],
            Msg::SetClearHistoryVerb => ["Delete", "삭제", "删除", "削除"],
            Msg::NoteClearArmed => [
                "Click again within 2 seconds to delete everything except pinned items",
                "2초 안에 다시 누르면 고정 항목을 뺀 모든 기록을 삭제합니다",
                "2 秒内再次点击将删除除固定项外的全部记录",
                "2 秒以内にもう一度押すと固定項目以外をすべて削除します",
            ],
            Msg::NoteClearDone => [
                "Deleted. Pinned items were kept",
                "삭제했습니다 · 고정 항목은 남겼습니다",
                "已删除。固定项已保留",
                "削除しました。固定項目は残しています",
            ],
            Msg::SetDiagLog => ["Diagnostic log", "진단 로그", "诊断日志", "診断ログ"],
            Msg::SetDiagLogDesc => [
                "Local only. Never sent anywhere",
                "로컬에만 남고 어디로도 보내지 않습니다",
                "仅保存在本地，不会发送",
                "ローカルのみ。外部へ送信しません",
            ],
            Msg::ValLangEn => ["English", "English", "English", "English"],
            Msg::ValLangKo => ["Korean", "한국어", "韩语", "韓国語"],
            Msg::ValLangZh => ["Chinese", "중국어", "简体中文", "中国語"],
            Msg::ValLangJa => ["Japanese", "일본어", "日语", "日本語"],
        }
    }
}

/// 번역 조회. 빈 문자열이면 **영어로 폴백**한다(누락이 화면을 비우지 않게).
#[must_use]
pub fn tr(lang: Lang, msg: Msg) -> &'static str {
    let row = msg.row();
    let s = row[lang.column()];
    if s.is_empty() {
        row[0]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 카탈로그 전수 — 새 `Msg`를 더하면 여기도 더한다(빈칸 검사가 그걸 강제한다).
    const ALL_MSG: [Msg; 234] = [
        Msg::AppName,
        Msg::SearchPlaceholder,
        Msg::EmptyHistory,
        Msg::NoMatches,
        Msg::KindText,
        Msg::KindRichText,
        Msg::KindImage,
        Msg::KindFiles,
        Msg::KindColor,
        Msg::ViewRich,
        Msg::ViewCompact,
        Msg::ViewPlain,
        Msg::ActionCopy,
        Msg::KindObject,
        Msg::ValItemsRange,
        Msg::ValTrayCountRange,
        Msg::PasteOriginal,
        Msg::PastePlain,
        Msg::PasteObject,
        Msg::PastePathOnly,
        Msg::ActionPastePlain,
        Msg::ActionPin,
        Msg::ActionDelete,
        Msg::ActionClearAll,
        Msg::ActionSettings,
        Msg::ActionAbout,
        Msg::ActionQuit,
        Msg::ActionOpenMain,
        Msg::ActionPauseCapture,
        Msg::StatusEncrypted,
        Msg::StatusLocalOnly,
        Msg::StatusCurrentClipboard,
        Msg::StatusWatchUnsupported,
        Msg::SyncTitle,
        Msg::SyncEnabled,
        Msg::SyncDevices,
        Msg::SyncPendingApproval,
        Msg::SyncEndToEnd,
        Msg::SizeSmall,
        Msg::SizeNormal,
        Msg::SizeLarge,
        Msg::SizeExtraLarge,
        Msg::SizeXLarge,
        Msg::ValMinutesRange,
        Msg::CustomInput,
        Msg::ListEmpty,
        Msg::SetUiFont,
        Msg::SetUiFontDesc,
        Msg::SetMonoFont,
        Msg::SetMonoFontDesc,
        Msg::SetMaxAge,
        Msg::SetMaxAgeDesc,
        Msg::SetMaxTotal,
        Msg::SetMaxTotalDesc,
        Msg::SystemDefaultFont,
        Msg::CatGeneral,
        Msg::CatShortcuts,
        Msg::SetHotkeyOpen,
        Msg::SetHotkeyOpenAlt,
        Msg::SetHotkeyPastePlain,
        Msg::SetHotkeyDesc,
        Msg::HotkeyNone,
        Msg::HotkeyTitle,
        Msg::HotkeyPrompt,
        Msg::HotkeyNeedMod,
        Msg::HotkeyRemove,
        Msg::HotkeyOk,
        Msg::HotkeyCancel,
        Msg::CatCapture,
        Msg::CatStorage,
        Msg::CatPrivacy,
        Msg::CatPaste,
        Msg::CatAppearance,
        Msg::CatSearch,
        Msg::CatSync,
        Msg::CatAdvanced,
        Msg::SetAutostart,
        Msg::SetAutostartDesc,
        Msg::SetLang,
        Msg::SetLangDesc,
        Msg::SetPopupAt,
        Msg::SetPopupAtDesc,
        Msg::ValCursor,
        Msg::ValScreenCenter,
        Msg::ValLastPos,
        Msg::SetViewMode,
        Msg::SetPopupView,
        Msg::SetPopupViewDesc,
        Msg::SetViewModeDesc,
        Msg::SetTheme,
        Msg::SetThemeDesc,
        Msg::ValSystem,
        Msg::ValDark,
        Msg::ValLight,
        Msg::SetCapText,
        Msg::SetCapImage,
        Msg::SetCapFiles,
        Msg::SetCapRich,
        Msg::SetCapNative,
        Msg::SetCapNativeDesc,
        Msg::SetMaxItems,
        Msg::SetMaxItemsDesc,
        Msg::SetSortBy,
        Msg::ValRecentCopy,
        Msg::ValFirstCopy,
        Msg::ValCopyCount,
        Msg::SetRespectMarks,
        Msg::SetRespectMarksDesc,
        Msg::SetConcealBrowserPw,
        Msg::SetConcealBrowserPwDesc,
        Msg::SetConcealUrls,
        Msg::SetConcealUrlsDesc,
        Msg::SetExcludeApps,
        Msg::SetExcludeAppsDesc,
        Msg::TrayOpen,
        Msg::TrayQuit,
        Msg::TraySettings,
        Msg::SetDockIcon,
        Msg::StSyncRevoke,
        Msg::StSyncDelete,
        Msg::StSyncSas,
        Msg::SetSyncRetry,
        Msg::SetSyncRetryDesc,
        Msg::SyncRetryNormal,
        Msg::SyncRetryPatient,
        Msg::SyncRetryEager,
        Msg::SyncRelayOff,
        Msg::StSyncLanOnly,
        Msg::SyncApprove,
        Msg::SyncApproveDesc,
        Msg::SyncApproveVerb,
        Msg::StSyncApproved,
        Msg::StSyncApproveNone,
        Msg::StSyncDevApproved,
        Msg::StSyncDevNeedsApproval,
        Msg::SetDockIconDesc,
        Msg::SearchHint,
        Msg::MainTitleSuffix,
        Msg::MainNoItems,
        Msg::MainNoMatch,
        Msg::PopupNoItems,
        Msg::StatusLine,
        Msg::StatusLineFiltered,
        Msg::TipPin,
        Msg::TipDelete,
        Msg::TipCopy,
        Msg::TipCopyPlain,
        Msg::TipAlwaysTop,
        Msg::TipPreview,
        Msg::TipCaptureStop,
        Msg::TipCaptureResume,
        Msg::TipSyncRelay,
        Msg::TipSyncLocal,
        Msg::TipSyncOff,
        Msg::TipSyncDown,
        Msg::MenuCopy,
        Msg::MenuCopyPlain,
        Msg::MenuCopyObject,
        Msg::MenuCopyPath,
        Msg::MenuPin,
        Msg::MenuUnpin,
        Msg::MenuEdit,
        Msg::MenuDelete,
        Msg::MenuCopyImage,
        Msg::MenuOrigin,
        Msg::DedupLabel,
        Msg::HintStack,
        Msg::SetSyncHandle,
        Msg::SetSyncDeviceName,
        Msg::SetSyncDeviceNameDesc,
        Msg::SetSyncDevices,
        Msg::SetSyncDevicesDesc,
        Msg::StSyncDevMe,
        Msg::StSyncDevOnline,
        Msg::StSyncDevAgo,
        Msg::SetSyncHandleDesc,
        Msg::SetSyncPass,
        Msg::SetSyncPassDesc,
        Msg::SetSyncRelay,
        Msg::SyncRelayDefault,
        Msg::SetSyncPort,
        Msg::SetSyncPortDesc,
        Msg::SyncPort47300,
        Msg::SyncTest,
        Msg::SyncTestDesc,
        Msg::SyncTestVerb,
        Msg::StSyncTesting,
        Msg::StSyncTestOk,
        Msg::StSyncTestFail,
        Msg::StSyncPassSuggested,
        Msg::SyncDisconnect,
        Msg::SyncDisconnectDesc,
        Msg::SyncDisconnectVerb,
        Msg::StSyncDisconnected,
        Msg::StSyncNeedIdentity,
        Msg::StSyncNotConnected,
        Msg::SetSyncRelayDesc,
        Msg::EditorHint,
        Msg::HintFiles,
        Msg::HintRich,
        Msg::HintImage,
        Msg::HintDefault,
        Msg::CtxSelectAll,
        Msg::CtxCopy,
        Msg::CtxCut,
        Msg::CtxPaste,
        Msg::WrapLabel,
        Msg::SetCloseToTray,
        Msg::SetCloseToTrayDesc,
        Msg::SetImagePreview,
        Msg::SetImagePreviewDesc,
        Msg::SetClearOnQuit,
        Msg::SetPasteAuto,
        Msg::SetPasteAutoDesc,
        Msg::SetPastePlainDefault,
        Msg::SetSearchMode,
        Msg::ValExact,
        Msg::ValFuzzy,
        Msg::ValRegex,
        Msg::SetSearchModeDesc,
        Msg::SetHangulCompose,
        Msg::SetSyncEnabledDesc,
        Msg::SetTrayRecent,
        Msg::SetDiagLog,
        Msg::SetDiagLogDesc,
        Msg::SetClearHistory,
        Msg::SetClearHistoryDesc,
        Msg::SetClearHistoryVerb,
        Msg::NoteClearArmed,
        Msg::NoteClearDone,
        Msg::ValLangEn,
        Msg::ValLangKo,
        Msg::ValLangZh,
        Msg::ValLangJa,
    ];

    #[test]
    fn code_roundtrip() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
        assert_eq!(Lang::from_code("xx"), None);
    }

    /// 열 번호와 `ALL` 순서가 어긋나면 **엉뚱한 언어가 나온다**.
    #[test]
    fn column_matches_all_order() {
        for (i, l) in Lang::ALL.iter().enumerate() {
            assert_eq!(l.column(), i, "{l:?} 의 열 번호가 ALL 순서와 다르다");
        }
    }

    #[test]
    fn current_lang_follows_set() {
        for (l, expect) in [
            (Lang::Ko, "복사"),
            (Lang::Zh, "复制"),
            (Lang::Ja, "コピー"),
            (Lang::En, "Copy"),
        ] {
            set_lang(l);
            assert_eq!(current_lang(), l);
            assert_eq!(tr(current_lang(), Msg::ActionCopy), expect);
        }
        set_lang(Lang::En); // 다른 테스트에 영향 주지 않게 복구
    }

    /// ★ 카탈로그에 빈 칸이 없어야 한다 — 있으면 화면이 비거나 영어로 새어 나간다.
    #[test]
    fn catalog_has_no_empty_cell() {
        for m in ALL_MSG {
            let row = m.row();
            assert_eq!(row.len(), Lang::ALL.len(), "언어 수와 열 수가 다르다");
            for (i, cell) in row.iter().enumerate() {
                assert!(!cell.is_empty(), "{m:?} 의 {i}번 언어가 비어 있다");
            }
        }
    }
}
