#!/usr/bin/env bash
# popup-cmd-e2e.sh — 팝업 선택 로직 자동 시험(11-16 · T-84 확장 · 셸 기동 명령 `popup.*`·`clip.*`·`shell.*`).
#   상주 셸(`nexa-clip tray`)을 격리 프로필로 띄우고 env `NEXA_CLIP_STARTUP_CMD`로
#   항목 추가 → 팝업 열기 → ↓ 이동 → (팝업이 열린 채) 새 항목 → 덤프를 차례로 구동한 뒤 popup.dump로 판정한다.
#   OS 입력 주입 없음 · **실제 클립보드를 쓰지 않는다**(clip.push는 캡처 경로에 직접 넣는다) — 다만 팝업 창이 1~2초 실제로 뜬다.
#   판정 대상 = 11-16 사용자 신고("↑/↓로 옮기면 원래 자리·맨 위로 돌아간다") 수정 79a38be.
# 쓰는 법: scripts/popup-cmd-e2e.sh [실행 파일(기본 target/debug/nexa-clip)]
#   먼저 `cargo build -p nexa-clip`. 프로필 = `popup-e2e`(실제 설정·이력과 분리) · 시작 전에 그 프로필 폴더를 비운다(이력 0에서 출발).
#   다른 인스턴스가 떠 있어도 된다(프로필별 단일 인스턴스) — 전역 단축키 등록 실패 경고는 정상.
# 종료 코드 = 실패 수(0 = 전부 통과).

set -uo pipefail
cd "$(dirname "$0")/.."

BIN="${1:-target/debug/nexa-clip}"
[ -x "$BIN" ] || [ -x "$BIN.exe" ] || { echo "실행 파일 없음: $BIN — 먼저 cargo build -p nexa-clip" >&2; exit 99; }
[ -x "$BIN" ] || BIN="$BIN.exe"

PROFILE=popup-e2e
X="$(cd "$(dirname "$BIN")" && pwd)/data/profiles/$PROFILE"   # Debug = 실행 파일 옆 data/(포터블)
# Windows Git Bash — 앱(Win32)이 열 수 있게 임시 폴더를 Windows 표기로(settings-cmd-e2e.sh와 같은 처방).
TMP_BASE="${TMPDIR:-/tmp}"
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) TMP_BASE="$(cygpath -m "${TMPDIR:-${TEMP:-/tmp}}")";; esac
D="$(mktemp -d "$TMP_BASE/nclip-popup-e2e.XXXXXX")"
case "$X" in */data/profiles/popup-e2e) rm -rf "$X";; esac   # 이력 0에서 출발(격리 프로필만 지운다)

PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); echo "  ✓ $1"; }
bad(){ FAIL=$((FAIL+1)); echo "  ✗ $1"; }
val(){ grep -m1 "^$2=" "$D/$1" 2>/dev/null | cut -d= -f2-; }   # val <덤프> <이름>

CMDS=(
  "clip.push:alpha" "clip.push:bravo" "clip.push:charlie"
  "popup.open" "popup.dump:$D/1.txt"                 # ① 열면 최신 항목 선택
  "popup.key:down" "popup.dump:$D/2.txt"             # ② ↓ 한 칸
  "clip.push:delta" "popup.dump:$D/3.txt"            # ③ 열린 채 새 항목 — 선택은 같은 항목에 남는다
  "popup.key:up" "popup.dump:$D/4.txt"               # ④ 그 자리에서 ↑ — 한 칸 위(맨 위로 튀지 않는다)
  "popup.close" "popup.open" "clip.push:echo" "popup.dump:$D/5.txt"   # ⑤ 손대지 않은 팝업은 새 항목을 따라간다
  "popup.close" "shell.quit"
)
JOINED="$(IFS=,; echo "${CMDS[*]}")"

echo "▶ 팝업 기동 명령 시험 — $BIN --profile $PROFILE tray · 덤프 $D"
NEXA_CLIP_STARTUP_CMD="$JOINED" "$BIN" --profile "$PROFILE" tray >"$D/console.log" 2>&1 &
PID=$!
for _ in $(seq 60); do kill -0 "$PID" 2>/dev/null || break; sleep 1; done
if kill -0 "$PID" 2>/dev/null; then kill "$PID"; bad "60초 안에 끝나지 않음(shell.quit 미도달)"; fi
wait "$PID"; RC=$?

# ① 열 때 선택 = 최신 항목(이력 0번)
[ "$(val 1.txt rows)" = "3" ] && [ "$(val 1.txt sel_label)" = "charlie" ] && [ "$(val 1.txt nav_moved)" = "false" ] \
  && ok "① open → 최신 항목 선택(charlie · rows=3)" \
  || bad "① rows=$(val 1.txt rows) sel_label=$(val 1.txt sel_label) nav_moved=$(val 1.txt nav_moved)"
# ② ↓ = 둘째 행
[ "$(val 2.txt sel)" = "1" ] && [ "$(val 2.txt sel_label)" = "bravo" ] && [ "$(val 2.txt nav_moved)" = "true" ] \
  && ok "② key:down → sel=1(bravo)" || bad "② sel=$(val 2.txt sel) sel_label=$(val 2.txt sel_label)"
# ③ 열린 채 새 항목 — 같은 항목(id)에 남고 행 번호만 밀린다(종전 결함 = 맨 위 delta로 되돌아감)
ID2=$(val 2.txt sel_id)
[ "$(val 3.txt rows)" = "4" ] && [ "$(val 3.txt sel_label)" = "bravo" ] && [ "$(val 3.txt sel_id)" = "$ID2" ] && [ "$(val 3.txt sel)" = "2" ] \
  && ok "③ 새 항목 뒤에도 선택 유지(bravo · sel 1→2 · id $ID2)" \
  || bad "③ rows=$(val 3.txt rows) sel=$(val 3.txt sel) sel_label=$(val 3.txt sel_label) sel_id=$(val 3.txt sel_id)(전 $ID2)"
# ④ 이어서 ↑ = 바로 위 행(charlie)
[ "$(val 4.txt sel)" = "1" ] && [ "$(val 4.txt sel_label)" = "charlie" ] \
  && ok "④ key:up → sel=1(charlie)" || bad "④ sel=$(val 4.txt sel) sel_label=$(val 4.txt sel_label)"
# ⑤ 손대지 않은 팝업 = 새 항목을 따라간다(08-28 계약 유지)
[ "$(val 5.txt rows)" = "5" ] && [ "$(val 5.txt sel)" = "0" ] && [ "$(val 5.txt sel_label)" = "echo" ] && [ "$(val 5.txt nav_moved)" = "false" ] \
  && ok "⑤ 손대지 않은 팝업 → 새 항목 선택(echo)" \
  || bad "⑤ rows=$(val 5.txt rows) sel=$(val 5.txt sel) sel_label=$(val 5.txt sel_label) nav_moved=$(val 5.txt nav_moved)"
# ⑥ 종료 · 명령 수 · 모르는 명령 0
NCMD=$(grep -c '기동 명령:' "$D/console.log"); NUNK=$(grep -c -E '모름|모르는' "$D/console.log")
[ "$RC" -eq 0 ] && [ "$NCMD" -eq "${#CMDS[@]}" ] && [ "$NUNK" -eq 0 ] \
  && ok "⑥ exit 0 · 기동 명령 ${NCMD}/${#CMDS[@]} · 모르는 명령 0" || bad "⑥ exit=$RC · 기동 명령 $NCMD/${#CMDS[@]} · 모르는 명령 $NUNK"

echo "결과: 통과 $PASS · 실패 $FAIL · 덤프·콘솔 = $D"
[ "$FAIL" -eq 0 ] || tail -15 "$D/console.log"
exit "$FAIL"
