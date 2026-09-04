#!/usr/bin/env bash
# mac-grant-accessibility.sh — 설치본 손쉬운 사용 권한 재부여(09-04 실기 박제):
#   macOS TCC 항목은 **처음 만들어질 때의 코드 요구사항(애드혹 = cdhash)** 을 계속 들고 있어
#   재서명(안정 신원으로 바꾼 첫 설치 포함) 뒤에는 토글을 껐다 켜도 안 맞는다(실측: 토글 ON인데
#   AXIsProcessTrusted=false). 처방 = 항목 **삭제 → 앱 재시작(기동 시 권한 대화상자가 새 항목 생성)
#   → 설정 창 토글 ON(UI 자동화) → 앱 자기 판정으로 검증**.
# 전제: 실행 터미널에 손쉬운 사용 권한(System Events로 시스템 설정 창을 조작한다).
set -uo pipefail
APP="/Applications/Nexa Clip.app"
BUNDLE_ID="io.github.sosomlab.nexa-clip"

self_status() {
    local out; out="$(mktemp)"
    open -n --stdout "$out" --stderr "$out" "$APP" --args status; sleep 3
    grep -E "paste inject" "$out"; rm -f "$out"
}

echo "1) 현재: $(self_status)"
if self_status | grep -q ": ok"; then echo "이미 허용됨 — 할 일 없음"; exit 0; fi

echo "2) TCC 항목 삭제"; tccutil reset Accessibility "$BUNDLE_ID"
echo "3) 앱 재시작(권한 대화상자로 새 항목 생성)"; pkill -x nexa-clip; sleep 1; open "$APP"; sleep 5
echo "4) 시스템 설정에서 토글 ON"
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"; sleep 3
osascript <<'AS' 2>&1
tell application "System Events"
    tell process "System Settings"
        set frontmost to true
        set allUI to entire contents of window 1
        set target to missing value
        repeat with e in allUI
            try
                if (class of e) is checkbox then
                    if (name of e as string) contains "Nexa" then set target to contents of e
                end if
            end try
        end repeat
        if target is missing value then return "   목록에 Nexa Clip 없음 — 권한 대화상자에서 '시스템 설정 열기'를 누르세요"
        if (value of target as string) is "0" then
            click target
            delay 4
        end if
        return "   토글 = " & (value of target as string)
    end tell
end tell
AS
sleep 1
echo "5) 검증: $(self_status)"
self_status | grep -q ": ok" && echo "★ 허용 완료 — scripts/mac-paste-e2e.sh 로 붙여넣기 3항 확인 가능" || { echo "★ 아직 미허용 — 설정 창에서 직접 켜 주세요(−/+ 재추가)"; exit 1; }
