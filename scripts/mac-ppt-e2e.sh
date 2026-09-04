#!/usr/bin/env bash
# mac-ppt-e2e.sh — PowerPoint(mac) 복사·붙여넣기 자동 실기(09-04 사용자 요청):
#   T1 글상자 안 텍스트(서식) — 복사 → 우리 판독(RichText·public.html) → 우리 경로 재게시(set_reps) →
#      새 글상자에 ⌘V → 텍스트·단어 색 보존 검증
#   T2 글상자 2개(Object) — 복사 → 판독(Object·GVML) → 재게시 → 새 슬라이드에 ⌘V → 도형 수·텍스트 검증
#
# 전제: PowerPoint가 프레젠테이션 하나를 연 채 실행 중 · 실행 터미널에 손쉬운 사용 권한(키 입력 · System Events)
#       · 릴리스 빌드(없으면 만든다). 슬라이드는 끝에 추가되며 지우지 않는다(눈으로 확인용 · 마지막에 번호 출력).
# ⚠️ 키 입력은 **물리 키 코드**(C=8 · V=9)로 보낸다 — `keystroke "c"`는 한글 입력기에서 "ㅊ"이 되어 무효(실측).
#    실행 중 클립보드가 바뀐다.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nexa-clip"
PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); echo "  ✓ $1"; }
bad()  { FAIL=$((FAIL+1)); echo "  ✗ $1"; }
check(){ if [[ "$2" == *"$3"* ]]; then ok "$1"; else bad "$1 — 기대 '$3' · 실제: $(echo "$2" | head -3 | tr '\n' ' ')"; fi; }

osascript -e 'tell application "Microsoft PowerPoint" to get version' >/dev/null 2>&1 \
    || { echo "PowerPoint가 실행 중이 아닙니다(프레젠테이션을 열어 두세요)" >&2; exit 1; }
[[ -x "$BIN" ]] || ( cd "$ROOT" && cargo build --release -p nexa-clip -p nclip-plat --example clip_roundtrip >/dev/null )
( cd "$ROOT" && cargo build --release -p nclip-plat --example clip_roundtrip 2>&1 | tail -1 )
ROUNDTRIP="$ROOT/target/release/examples/clip_roundtrip"

TEXT="가나다 123 ABC"

echo "── T1 글상자 안 텍스트(서식) ──"
S1=$(osascript <<AS 2>&1
tell application "Microsoft PowerPoint"
    activate
    set pres to active presentation
    set sl to make new slide at end of pres with properties {layout:slide layout blank}
    set idx to slide index of sl
    set slide index of active window to idx
    set tb to make new text box at end of sl with properties {left position:60, top:60, width:600, height:100}
    set tr to text range of text frame of tb
    set content of tr to "$TEXT"
    set font color of font of word 2 of tr to {255, 0, 0}
    set font color of font of word 3 of tr to {33, 95, 154}
    select text range of text frame of tb
    delay 0.4
    tell application "System Events" to tell process "Microsoft PowerPoint" to key code 8 using command down
    delay 0.8
    return idx
end tell
AS
)
echo "  슬라이드 $S1 생성 · 텍스트 복사(⌘C)"
P=$("$BIN" peek 2>&1)
check "판독 종류 = RichText" "$P" "RichText"
check "public.html 표현 존재" "$P" "public.html"
R=$("$ROUNDTRIP" 2>&1)
check "우리 경로 재게시(set_reps) 왕복 동일" "$R" "전부 동일"
V1=$(osascript <<AS 2>&1
with timeout of 40 seconds
tell application "Microsoft PowerPoint"
    activate
    set sl to slide $S1 of active presentation
    set tb2 to make new text box at end of sl with properties {left position:60, top:220, width:600, height:100}
    select text range of text frame of tb2
    delay 0.4
    tell application "System Events" to tell process "Microsoft PowerPoint" to key code 9 using command down
    delay 1.2
    set tr2 to text range of text frame of tb2
    set c2 to font color of font of word 2 of tr2
    set c3 to font color of font of word 3 of tr2
    return (content of tr2) & "|" & (item 1 of c2) & "," & (item 2 of c2) & "," & (item 3 of c2) & "|" & (item 1 of c3) & "," & (item 2 of c3) & "," & (item 3 of c3)
end tell
end timeout
AS
)
echo "  붙여넣기 결과: $V1"
check "붙여넣은 텍스트 일치" "$V1" "$TEXT"
check "단어 '123' 빨강 보존" "$V1" "|255,0,0|"
check "단어 'ABC' 파랑 보존" "$V1" "|33,95,154"

echo "── T2 글상자 2개(Object) ──"
S2=$(osascript <<AS 2>&1
with timeout of 30 seconds
tell application "Microsoft PowerPoint"
    activate
    set pres to active presentation
    set sl to make new slide at end of pres with properties {layout:slide layout blank}
    set idx to slide index of sl
    set slide index of active window to idx
    set a to make new text box at end of sl with properties {left position:60, top:60, width:400, height:80}
    set content of text range of text frame of a to "첫째 상자"
    set b to make new text box at end of sl with properties {left position:60, top:180, width:400, height:80}
    set content of text range of text frame of b to "둘째 상자"
end tell
delay 0.5
-- 도형 다중 선택은 AppleScript select가 거부한다(실측) — 편집 종료(ESC) 후 ⌘A(슬라이드 전체) → ⌘C.
tell application "System Events" to tell process "Microsoft PowerPoint"
    key code 53
    delay 0.2
    key code 53
    delay 0.3
    key code 0 using command down
    delay 0.5
    key code 8 using command down
end tell
delay 1
return idx
end timeout
AS
)
echo "  슬라이드 $S2 생성 · 글상자 2개 복사(⌘C)"
P2=$("$BIN" peek 2>&1)
check "판독 종류 = Object" "$P2" "Object"
check "GVML(PPT 도형) 표현 존재" "$P2" "GVML"
R2=$("$ROUNDTRIP" 2>&1)
check "재게시 왕복 동일" "$R2" "전부 동일"
V2=$(osascript <<AS 2>&1
with timeout of 40 seconds
tell application "Microsoft PowerPoint"
    activate
    set pres to active presentation
    set sl to make new slide at end of pres with properties {layout:slide layout blank}
    set idx to slide index of sl
    set slide index of active window to idx
end tell
delay 0.6
tell application "System Events" to tell process "Microsoft PowerPoint"
    key code 53
    delay 0.2
    key code 9 using command down
end tell
delay 2
tell application "Microsoft PowerPoint"
    set sl to slide idx of active presentation
    set n to count of shapes of sl
    set txt to ""
    repeat with i from 1 to n
        try
            set txt to txt & (content of text range of text frame of shape i of sl) & ";"
        end try
    end repeat
    return "slide " & idx & " shapes=" & n & " text=" & txt
end tell
end timeout
AS
)
echo "  붙여넣기 결과: $V2"
check "도형 2개 붙음" "$V2" "shapes=2"
check "둘째 상자 텍스트 보존" "$V2" "둘째 상자"

echo "── 결과: 통과 $PASS · 실패 $FAIL (슬라이드 ${S1}·${S2}·+1은 눈으로 확인 후 지우세요) ──"
[[ $FAIL -eq 0 ]]
