#!/usr/bin/env bash
# settings-cmd-e2e.sh — 설정 창 기동 명령(T-84 · 11-08) 자동 시험.
#   단독 설정 창(`nexa-clip settings`)을 격리 프로필로 띄우고 env `NEXA_CLIP_STARTUP_CMD`로
#   카테고리 이동 · 필터 · 검색 · 키 · 휠 · 값 변경 · 내보내기/가져오기를 차례로 구동한 뒤
#   prefs.dump 덤프로 판정한다. OS 입력 주입 없음(클립보드·키 주입 0) — 다만 창이 1~2초 실제로 뜬다.
# 쓰는 법: scripts/settings-cmd-e2e.sh [실행 파일(기본 target/debug/nexa-clip)]
#   먼저 `cargo build -p nexa-clip`. 프로필 = `cmd-e2e`(실제 설정·dev 프로필과 분리) · 끝나면 그 프로필
#   settings.cfg를 시험 전 상태로 되돌린다.
# 종료 코드 = 실패 수(0 = 전부 통과).

set -uo pipefail
cd "$(dirname "$0")/.."

BIN="${1:-target/debug/nexa-clip}"
[ -x "$BIN" ] || [ -x "$BIN.exe" ] || { echo "실행 파일 없음: $BIN — 먼저 cargo build -p nexa-clip" >&2; exit 99; }
[ -x "$BIN" ] || BIN="$BIN.exe"

PROFILE=cmd-e2e
X="$(cd "$(dirname "$BIN")" && pwd)/data/profiles/$PROFILE"   # Debug = 실행 파일 옆 data/(포터블)
D="$(mktemp -d "${TMPDIR:-/tmp}/nclip-cmd-e2e.XXXXXX")"
mkdir -p "$X"
CFG_BAK=""
if [ -f "$X/settings.cfg" ]; then CFG_BAK="$D/settings.cfg.before"; cp "$X/settings.cfg" "$CFG_BAK"; fi

PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); echo "  ✓ $1"; }
bad(){ FAIL=$((FAIL+1)); echo "  ✗ $1"; }
val(){ grep -m1 -o "$2=[^ ]*" "$D/$1" 2>/dev/null | cut -d= -f2-; }   # val <덤프> <이름>

# ★ 화면 언어 고정(11-14) — 아래 판정이 영어 토스트 문구를 grep한다. 시스템 로캘이 ko면 ⑥⑦이 거짓 실패하던 것
#   (협업 V1 11-14) → 격리 프로필 안에서 ⑥ 직전에 app.lang=en(OS별 로캘 경로와 무관 · 3-OS 동일).
#   ② "변경만" 판정(cards=0) 뒤에 둔다 — 앞에 두면 app.lang이 변경 카드로 남는다.
CMDS=(
  "prefs.goto:ui.toast_ms"   "prefs.dump:$D/1.txt"
  "prefs.goto:scroll.fast"   "prefs.dump:$D/1b.txt"
  "prefs.filter:changed"     "prefs.dump:$D/2.txt"
  "prefs.filter:all"         "prefs.advanced:on"
  "prefs.search:플래시"       "prefs.dump:$D/3.txt"
  "prefs.search:"            "prefs.key:end"           "prefs.dump:$D/4.txt"
  "prefs.key:home"           "prefs.wheel:-3"          "prefs.dump:$D/5.txt"
  "prefs.set:app.lang=en"
  "prefs.set:ui.flash_ms=900" "prefs.export:$D/e2e.json" "prefs.dump:$D/6.txt"
  "prefs.import:$D/e2e.json" "prefs.dump:$D/7.txt"
  "prefs.sleep:300"          "prefs.close"
)
JOINED="$(IFS=,; echo "${CMDS[*]}")"

echo "▶ 설정 창 기동 명령 시험 — $BIN --profile $PROFILE settings · 덤프 $D"
NEXA_CLIP_STARTUP_CMD="$JOINED" "$BIN" --profile "$PROFILE" settings >"$D/console.log" 2>&1 &
PID=$!
for _ in $(seq 60); do kill -0 "$PID" 2>/dev/null || break; sleep 1; done
if kill -0 "$PID" 2>/dev/null; then kill "$PID"; bad "60초 안에 끝나지 않음(prefs.close 미도달)"; fi
wait "$PID"; RC=$?

# ① goto — 대상 카드가 맨 위 · 카테고리 첫 카드가 아니면 scroll > 0
[ "$(val 1.txt top_card)" = "ui.toast_ms" ] && ok "① goto ui.toast_ms → top_card" || bad "① top_card=$(val 1.txt top_card)"
[ "$(val 1b.txt top_card)" = "scroll.fast" ] && [ "$(val 1b.txt scroll)" -gt 0 ] 2>/dev/null \
  && ok "① goto scroll.fast → top_card · scroll=$(val 1b.txt scroll)" \
  || bad "① goto scroll.fast — top_card=$(val 1b.txt top_card) scroll=$(val 1b.txt scroll)"
# ② 변경만 보기 — 시험 프로필은 이 시점까지 바꾼 값이 없으니 cards=0(정보·보고 행은 '변경'이 아니다 · 11-08)
[ "$(val 2.txt changed_only)" = "true" ] && [ "$(val 2.txt cards)" = "0" ] \
  && ok "② filter:changed → changed_only=true · cards=0" \
  || bad "② changed_only=$(val 2.txt changed_only) · cards=$(val 2.txt cards)($(grep -E '^[a-z_]+\.[a-z_.]+$' "$D/2.txt" | tr '\n' ' '))"
# ③ 검색
[ "$(val 3.txt query)" = "플래시" ] && grep -q '^ui\.flash_' "$D/3.txt" && ok "③ search 플래시 → ui.flash_* 카드" || bad "③ query=$(val 3.txt query)"
# ④ End = 끝까지
S4=$(val 4.txt scroll); C4=$(val 4.txt content_h); V4=$(val 4.txt viewport_h)
[ -n "$S4" ] && [ "$S4" -eq $((C4 - V4)) ] 2>/dev/null && ok "④ key:end → scroll=$S4 = content_h−viewport_h" || bad "④ scroll=$S4 content_h=$C4 viewport_h=$V4"
# ⑤ Home 뒤 휠 3노치(아래)
S5=$(val 5.txt scroll)
[ -n "$S5" ] && [ "$S5" -gt 0 ] && [ "$S5" -lt "${S4:-0}" ] 2>/dev/null && ok "⑤ home + wheel:-3 → scroll=$S5" || bad "⑤ scroll=$S5 (④=$S4)"
# ⑥ 값 변경 + 내보내기
grep -q '^toast=Exported settings' "$D/6.txt" && [ -f "$D/e2e.json" ] && grep -q '"flash_ms": *900' "$D/e2e.json" \
  && ok "⑥ set ui.flash_ms=900 · export → 토스트 · JSON에 flash_ms 900" || bad "⑥ export — $(grep '^toast=' "$D/6.txt" | tr '\n' ' ')"
# ⑦ 가져오기
grep -q '^toast=Imported settings' "$D/7.txt" && ok "⑦ import → $(grep '^toast=Imported' "$D/7.txt")" || bad "⑦ import 토스트 없음"
# ⑧ 종료 · 명령 수 · 모르는 명령 0
NCMD=$(grep -c '기동 명령:' "$D/console.log"); NUNK=$(grep -c -E '모름|모르는' "$D/console.log")
[ "$RC" -eq 0 ] && [ "$NCMD" -eq "${#CMDS[@]}" ] && [ "$NUNK" -eq 0 ] \
  && ok "⑧ exit 0 · 기동 명령 ${NCMD}/${#CMDS[@]} · 모르는 명령 0" || bad "⑧ exit=$RC · 기동 명령 $NCMD/${#CMDS[@]} · 모르는 명령 $NUNK"

# 복원 — 시험 프로필 설정을 시험 전 상태로
if [ -n "$CFG_BAK" ]; then cp "$CFG_BAK" "$X/settings.cfg"; else rm -f "$X/settings.cfg"; fi

echo "결과: 통과 $PASS · 실패 $FAIL · 덤프·콘솔 = $D"
[ "$FAIL" -eq 0 ] || tail -15 "$D/console.log"
exit "$FAIL"
