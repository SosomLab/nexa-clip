#!/usr/bin/env bash
# mac-paste-e2e.sh — 설치본 붙여넣기 자동 실기(09-04 사용자 "붙여넣기 테스트 좀 해줘"):
#   사용자 경로 그대로 — 복사(pbcopy → 감시가 이력 맨 위에) → TextEdit 새 문서 → 전역 단축키
#   ⇧⌥C(팝업) → Enter(재적재 + 포커스 복원 + ⌘V 주입) → TextEdit 본문에 붙었는지 검증.
#   P2 ⇧Enter(평문 모드) · P3 ⇧⌥X(전역 평문 붙여넣기)도 같은 방식.
# 전제: 설치본(/Applications/Nexa Clip.app) 상주 · 실행 터미널에 손쉬운 사용(System Events 키 입력).
# 실패 시 설치본 **자기 권한 판정**(open --args status)을 함께 찍는다 — "needs permission"이면
# 손쉬운 사용 토글이 원인(재서명 뒤 토글 껐다 켜기 · 안정 신원은 scripts/mac-dev-cert.sh).
# ⚠️ 키는 물리 키 코드(C=8 · X=7 · Return=36) — keystroke 문자는 한글 입력기에서 무효.

set -uo pipefail
APP="/Applications/Nexa Clip.app"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); echo "  ✓ $1"; }
bad(){ FAIL=$((FAIL+1)); echo "  ✗ $1"; }

pgrep -x nexa-clip >/dev/null || { echo "설치본이 실행 중이 아닙니다: open \"$APP\"" >&2; exit 1; }

self_status() {
    local out; out="$(mktemp)"
    open -n --stdout "$out" --stderr "$out" "$APP" --args status; sleep 2
    grep -E "paste inject" "$out" || echo "  (status 출력 없음)"
    rm -f "$out"
}

# TextEdit 새 문서(본문 비움) · 프런트
te_new() {
    osascript <<'AS' >/dev/null 2>&1
tell application "TextEdit"
    activate
    set d to make new document
    set text of d to ""
end tell
AS
    sleep 0.8
}
te_text() { osascript -e 'tell application "TextEdit" to get text of document 1' 2>/dev/null; }
te_close() { osascript -e 'tell application "TextEdit" to close document 1 saving no' >/dev/null 2>&1; }

run_case() {  # $1 이름 · $2 마커 · $3 = 키 시퀀스(AppleScript 조각)
    local name="$1" marker="$2" keys="$3"
    printf '%s' "$marker" | pbcopy; sleep 1.2      # 감시 폴링(200ms) + settle 뒤 이력 맨 위
    te_new
    osascript <<AS >/dev/null 2>&1
tell application "System Events" to tell process "TextEdit"
$keys
end tell
AS
    sleep 2
    local got; got="$(te_text)"
    if [[ "$got" == *"$marker"* ]]; then ok "$name — 붙음"; else bad "$name — 안 붙음(본문: '${got:0:40}')"; fi
    te_close
}

# ── 설정의 실제 단축키(key.open · key.paste_plain)를 키 코드·수식으로(09-04 실기: 하드코딩 ⇧⌥C가 사용자 설정 ⇧⌘C와 달라 'C'만 타이핑됨)
CFG="$HOME/Library/Application Support/nexa-clip/settings.cfg"
keycode_of() {  # 마지막 토큰(문자) → mac 키 코드
    case "$(echo "$1" | tr '[:lower:]' '[:upper:]')" in
        A) echo 0;; S) echo 1;; D) echo 2;; F) echo 3;; H) echo 4;; G) echo 5;; Z) echo 6;; X) echo 7;; C) echo 8;; V) echo 9;;
        B) echo 11;; Q) echo 12;; W) echo 13;; E) echo 14;; R) echo 15;; Y) echo 16;; T) echo 17;; 1) echo 18;; 2) echo 19;; 3) echo 20;;
        4) echo 21;; 6) echo 22;; 5) echo 23;; 9) echo 25;; 7) echo 26;; 8) echo 28;; 0) echo 29;; O) echo 31;; U) echo 32;; I) echo 34;;
        P) echo 35;; L) echo 37;; J) echo 38;; K) echo 40;; N) echo 45;; M) echo 46;; *) echo "";;
    esac
}
mods_of() {  # "Shift+Win+C" → "{shift down, command down}"
    local m=(); local IFS='+'; read -ra parts <<< "$1"
    for p in "${parts[@]}"; do
        case "$p" in Shift) m+=("shift down");; Win|Cmd|Meta|Super) m+=("command down");; Alt|Option) m+=("option down");; Ctrl|Control) m+=("control down");; esac
    done
    local out=""; for x in "${m[@]}"; do out="${out:+$out, }$x"; done; echo "{$out}"
}
hk_open="$(sed -n 's/^key\.open=//p' "$CFG" | head -1)"; hk_open="${hk_open:-Shift+Alt+C}"
hk_plain="$(sed -n 's/^key\.paste_plain=//p' "$CFG" | head -1)"; hk_plain="${hk_plain:-Shift+Alt+X}"
kc_open="$(keycode_of "${hk_open##*+}")"; md_open="$(mods_of "$hk_open")"
kc_plain="$(keycode_of "${hk_plain##*+}")"; md_plain="$(mods_of "$hk_plain")"
echo "── 붙여넣기 E2E(설치본) — 팝업=$hk_open(code $kc_open) · 평문=$hk_plain(code $kc_plain) ──"
M="NEXA-PASTE-$(date +%H%M%S)"
run_case "P1 팝업 → Enter(원본)" "$M-1" "
    key code $kc_open using $md_open
    delay 1.2
    key code 36"
run_case "P2 팝업 → ⇧Enter(평문)" "$M-2" "
    key code $kc_open using $md_open
    delay 1.2
    key code 36 using shift down"
if [[ -n "$kc_plain" ]]; then
run_case "P3 전역 평문 붙여넣기($hk_plain)" "$M-3" "
    key code $kc_plain using $md_plain"
fi

echo "── 결과: 통과 $PASS · 실패 $FAIL ──"
if [[ $FAIL -gt 0 ]]; then
    echo "── 설치본 자기 권한 판정 ──"; self_status
fi
[[ $FAIL -eq 0 ]]
