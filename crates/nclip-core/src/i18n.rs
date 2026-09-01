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
    /// 시스템 기본 글꼴.
    SystemDefaultFont,
    /// 설정 카테고리: 일반.
    CatGeneral,
    /// 설정 카테고리: 단축키.
    CatShortcuts,
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
            Msg::SystemDefaultFont => ["System default", "시스템 기본", "系统默认", "システム標準"],
            Msg::CatGeneral => ["General", "일반", "常规", "一般"],
            Msg::CatShortcuts => ["Shortcuts", "단축키", "快捷键", "ショートカット"],
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
            Msg::TrayOpen => ["Open", "열기", "打开", "開く"],
            Msg::TrayQuit => ["Quit", "종료", "退出", "終了"],
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
    const ALL_MSG: [Msg; 118] = [
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
        Msg::SystemDefaultFont,
        Msg::CatGeneral,
        Msg::CatShortcuts,
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
        Msg::SetHangulCompose,
        Msg::SetSyncEnabledDesc,
        Msg::SetTrayRecent,
        Msg::SetDiagLog,
        Msg::SetDiagLogDesc,
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
