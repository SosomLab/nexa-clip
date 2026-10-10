#!/usr/bin/env bash
# linux-sync-e2e.sh — 같은 PC 두 인스턴스(설치본 + Debug) 사이의 **전파 동작을 프로그램별 시나리오로 구별**해 판정한다(10-10 · T-71).
#
# 왜 구별이 어려운가: 두 인스턴스가 같은 클립보드를 함께 감시해 한 번 복사하면 둘 다 "로컬 캡처"로 잡고,
# 서로 전파해 "⇄ 수신" 항목이 또 생기고, 수신 항목을 게시하면 상대가 그것을 되읽고, VMware 다리가 몇 초 뒤 되쓴다.
# 구별 방법: ① 시나리오마다 **고유 표식 문자열**(E2E-SYNC-<이름>-<시각>) ② 로그 키워드로 역할을 가른다 —
#   "이력: New(⇄ 없음)" = 로컬 캡처 · "동기화: 항목 전파 → 기기 N대" = 송신 · "동기화: ← <기기> 항목 수신" = 수신 ·
#   "이력: New" (수신 직후) = ⇄ 항목(기기별 별도 항목 = 설계) · 그 뒤 "이력: Promoted" = 되읽기/되쓰기 에코 흡수.
#   ③ Debug 로그는 시나리오 시작 시점의 **줄 오프셋**으로 자르고, 설치본은 journalctl의 **시각**으로 자른다.
#   ④ Debug를 NEXA_CLIP_DIAG=1 로 띄우면 "이력:" 줄에 미리보기(표식)가 보여 짝을 확정할 수 있다(T-69 — 기본은 종류만).
#
# 판정(Debug · 수정 코드 0.2.1): 표식 하나에 대해 New == 2(로컬 1 + ⇄ 1) · 수신 ≥ 1 · 수신 뒤 New == 0 · Promoted ≥ 1.
#   "동기화: 전파 안 함 — 같은 내용을 <기기>에서 받은 항목이 이력에 있음" = 재전파 가드(10-10 · 수신 내용 되돌려 보내기 차단) 횟수도 센다.
#   수정 전 코드라면 "수신 뒤 New"가 1 더 찍힌다(T-71) — 설치본(0.1.9)은 **대조군**으로 같은 지표를 "관찰"로만 적는다.
#
# 프로그램별 시나리오(= 복사를 일으키는 프로그램/백엔드가 다르다 → 출처 앱·표현 이름이 달라진다):
#   S1 gtk-x11-text    : python3 GTK3 · GDK_BACKEND=x11 · set_text              (출처 python3 · 표현 text/plain 계열)
#   S2 gtk-wayland-text: python3 GTK3 · GDK_BACKEND=wayland · set_text          (Mutter가 X 클립보드로 다리 → 출처 미상 가능)
#   S3 gtk-x11-image   : python3 GTK3 · GDK_BACKEND=x11 · set_image(PNG)        (image/png → 전파 image · 되쓰기 그림 흡수)
#   (앱 UI 조작이 필요한 gnome-text-editor 등 Wayland 네이티브 앱은 xdotool이 닿지 않아 사람 실기 — docs/21 §6)
#
# 사용:  scripts/linux-sync-e2e.sh [S1 S2 S3 …]     (인자 없으면 전부)
#   환경: NCLIP_SYNC_PEER=kiros33@lin(상대 기기 표시 이름 · 기본) · NCLIP_SYNC_SETTLE=10(복사 뒤 대기 초 · 다리 되쓰기 2~3초 + 전파·게시·되읽기 여유)
#         NCLIP_SYNC_INSTALLED_PID=<pid>(설치본 · 기본 = /usr/bin/nexa-clip 프로세스 자동)
# ⚠️ 클립보드를 덮어쓴다(복사해 둔 것은 먼저 붙여 넣을 것) · 사용자 창·문서는 건드리지 않는다(자체 python 프로세스만).
# 전제: Debug(target/debug/nexa-clip tray)와 설치본이 둘 다 떠 있고, 양쪽 devices.txt에서 서로 'A'(승인)여야 전파가 된다.
set -u
REPO="$(cd "$(dirname "$0")/.." && pwd)"
LOG="$REPO/target/debug/nexa-clip.log"
PEER="${NCLIP_SYNC_PEER:-kiros33@lin}"
SETTLE="${NCLIP_SYNC_SETTLE:-10}"
OUT="$REPO/target/e2e"; mkdir -p "$OUT"
STAMP="$(date +%m%d-%H%M%S)"
REPORT="$OUT/sync-$STAMP.txt"
PASS=0; FAIL=0; HOLDERS=()

say() { echo "$*" | tee -a "$REPORT"; }
cleanup() { for p in "${HOLDERS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done; }
trap cleanup EXIT

# ---------- 0. 전제 ----------
DBG_PID=$(pgrep -f "$REPO/target/debug/nexa-clip tray" | head -1 || true)
[ -z "$DBG_PID" ] && DBG_PID=$(pgrep -f "target/debug/nexa-clip tray" | head -1 || true)
INST_PID="${NCLIP_SYNC_INSTALLED_PID:-$(pgrep -f '^/usr/bin/nexa-clip' | head -1 || true)}"
say "### linux-sync-e2e $STAMP · Debug pid=${DBG_PID:-없음} · 설치본 pid=${INST_PID:-없음} · peer=$PEER · settle=${SETTLE}s"
[ -z "$DBG_PID" ] && { say "✗ Debug 인스턴스가 없다(scripts/dev-restart.sh --diag 먼저)"; exit 2; }
[ -f "$LOG" ] || { say "✗ Debug 로그 없음: $LOG"; exit 2; }
grep -q "DIAG\|\[diag\]" "$LOG" 2>/dev/null || say "ℹ️ Debug 로그에 진단 표식이 없다 — NEXA_CLIP_DIAG=1 이 아니면 '이력:' 줄에 표식 미리보기가 안 보여 짝은 순서·개수로만 판정한다"
chk_approved() { # $1 devices.txt · $2 이름 → 'A'면 0
  grep -q "^v2 [0-9a-f]\{64\} [0-9]* [0-9]* [a-z-]* A $2\$" "$1" 2>/dev/null
}
chk_approved "$REPO/target/debug/data/devices.txt" "$PEER" && say "✓ Debug → $PEER 승인됨" || say "⚠️ Debug 쪽 devices.txt에 $PEER 승인(A) 없음 — 수신이 'device not approved'로 막힌다"
INST_DEV="$HOME/.config/nexa-clip/devices.txt"
DBG_NAME=$(grep -o '^sync.device_name=.*' "$REPO/target/debug/data/settings.cfg" | cut -d= -f2)
chk_approved "$INST_DEV" "$DBG_NAME" && say "✓ 설치본 → $DBG_NAME 승인됨" || say "⚠️ 설치본 devices.txt에 $DBG_NAME 승인(A) 없음 — 설치본이 전파하지 않는다"

# ---------- 1. 복사 도구(자체 python 프로세스 · 셀렉션 주인으로 상주) ----------
copier() { # $1 backend(x11|wayland) · $2 kind(text|image) · $3 payload(text 또는 png 경로) → 백그라운드 pid
  # stdout/stderr를 떼어 둔다 — 안 떼면 `pid=$(copier …)`가 python이 끝날 때까지(최대 120초) 막힌다(협업 세션 사전 점검 10-10).
  GDK_BACKEND="$1" python3 - "$2" "$3" >/dev/null 2>&1 <<'PY' &
import sys, gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk, GdkPixbuf, GLib
kind, payload = sys.argv[1], sys.argv[2]
cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
if kind == 'text':
    cb.set_text(payload, -1)
else:
    cb.set_image(GdkPixbuf.Pixbuf.new_from_file(payload))
GLib.timeout_add_seconds(120, Gtk.main_quit)   # 주인으로 최대 120초 상주(시나리오 끝에 kill)
Gtk.main()
PY
  echo $!
}

# ---------- 2. 판정 ----------
count() { grep -c -- "$1" <<<"$2" 2>/dev/null || true; }
judge() { # $1 이름 · $2 Debug 로그 조각 · $3 설치본 저널 조각 · $4 표식
  local name="$1" dbg="$2" inst="$3" mark="$4"
  local d_new d_prom d_send d_recv d_after
  d_new=$(count "이력: New" "$dbg"); d_prom=$(count "이력: Promoted" "$dbg")
  d_send=$(count "항목 전파 → 기기" "$dbg"); d_recv=$(count "← $PEER 항목 수신" "$dbg")
  # 수신 직후 첫 New = ⇄ 항목(기기별 별도 항목 · 설계) → 그 뒤의 New만 "되읽기/되쓰기가 새 항목이 된 것"(T-71).
  d_after=$(awk -v p="← $PEER 항목 수신" 'f && /이력: New/ {if (skip) skip=0; else n++} index($0,p) {f=1; skip=1} END{print n+0}' <<<"$dbg")
  # 송신은 "→ <기기>(…) 항목" 줄을 **대상 이름으로** 센다 — "항목 전파 → 기기 N대"는 mac 등 다른 승인 기기까지 합친 수라
  # 두 인스턴스 사이의 송신과 구별이 안 된다(협업 세션 관찰 10-10). 설치본의 "승인 전이라 버림" = 거부(수신 0의 원인).
  local d_to_peer d_guard; d_to_peer=$(count "→ $PEER(" "$dbg"); d_guard=$(count "전파 안 함 — 같은 내용을" "$dbg")
  local i_new i_prom i_send i_recv i_after i_to_dbg i_to_other i_reject
  i_new=$(count "이력: New" "$inst"); i_prom=$(count "이력: Promoted" "$inst")
  i_send=$(count "항목 전파 → 기기" "$inst"); i_recv=$(count "항목 수신" "$inst")
  i_to_dbg=$(count "→ $DBG_NAME(" "$inst"); i_reject=$(count "승인 전이라 버림" "$inst")
  i_to_other=$(grep -o "→ [^( ]*(" <<<"$inst" | grep -v -F "→ $DBG_NAME(" | sort | uniq -c | awk '{printf "%s×%s ", $3, $1}' | sed 's/(//g; s/ $//')
  i_after=$(awk 'f && /이력: New/ {if (skip) skip=0; else n++} /항목 수신/ {f=1; skip=1} END{print n+0}' <<<"$inst")
  say "  [Debug 0.2.1·수정] New=$d_new Promoted=$d_prom 전파=$d_send(→$PEER $d_to_peer) 수신=$d_recv 수신뒤New=$d_after 재전파가드=$d_guard"
  say "  [설치본 0.1.9·대조] New=$i_new Promoted=$i_prom 전파=$i_send(→$DBG_NAME $i_to_dbg · 다른 기기 ${i_to_other:-0}) 수신=$i_recv 거부=$i_reject 수신뒤New=$i_after (관찰 · T-71 수정 전 코드면 수신뒤New≥1)"
  [ -n "$i_to_other" ] && say "  ⚠️ 설치본이 표식을 다른 승인 기기로도 보냈다: $i_to_other — 그 기기 이력에 시험 표식이 들어간다"
  if [ -n "$mark" ] && grep -q -- "$mark" <<<"$dbg"; then say "  표식 확인: Debug 로그에 '$mark' 보임(DIAG)"; fi
  local ok=1 why=()
  [ "$d_recv" -ge 1 ] || { ok=0; why+=("수신 0$( [ "${i_reject:-0}" -ge 1 ] && echo "(설치본이 승인 전이라 버림 $i_reject)")"); }
  [ "$d_send" -ge 1 ] || { ok=0; why+=("전파 0"); }
  [ "$d_new" -eq 2 ] || { ok=0; why+=("New=$d_new(기대 2 = 로컬+⇄)"); }
  if [ "$d_after" -ne 0 ]; then
    if [ "$d_guard" -ge 1 ]; then why+=("관찰: 수신 뒤 New=$d_after 이지만 재전파 가드가 막음(상대로 안 감)"); else ok=0; why+=("수신 뒤 New=$d_after(T-71 재발 · 가드 0 → 상대로 되돌아감)"); fi
  fi
  [ "$d_prom" -ge 1 ] || { ok=0; why+=("Promoted 0(에코 흡수 없음)"); }
  if [ $ok = 1 ]; then PASS=$((PASS+1)); say "  ✅ $name PASS"; else FAIL=$((FAIL+1)); say "  ❌ $name FAIL — ${why[*]}"; fi
}

run_case() { # $1 이름 · $2 backend · $3 kind · $4 payload(텍스트 표식 또는 png)
  local name="$1" backend="$2" kind="$3" payload="$4"
  local mark=""; [ "$kind" = text ] && mark="$payload"
  say ""; say "===== $name ($backend · $kind) ====="
  local off; off=$(wc -l < "$LOG"); local t0; t0=$(date +%s)
  local pid; pid=$(copier "$backend" "$kind" "$payload"); HOLDERS+=("$pid")
  sleep "$SETTLE"
  local dbg inst
  dbg=$(tail -n +"$((off+1))" "$LOG")
  inst=$(journalctl --user -o short-iso --since "@$t0" --no-pager 2>/dev/null | grep -F "[${INST_PID:-0}]" || true)
  { echo "--- Debug 로그($name)"; echo "$dbg"; echo "--- 설치본 저널($name)"; echo "$inst"; } >> "$OUT/sync-$STAMP-$name.log"
  judge "$name" "$dbg" "$inst" "$mark"
  kill "$pid" 2>/dev/null; sleep 1
}

CASES=("$@"); [ ${#CASES[@]} -eq 0 ] && CASES=(S1 S2 S3)
PNG="$OUT/sync-$STAMP.png"
for c in "${CASES[@]}"; do
  case "$c" in
    S1) run_case S1-gtk-x11-text x11 text "E2E-SYNC-S1-$STAMP" ;;
    S2) run_case S2-gtk-wayland-text wayland text "E2E-SYNC-S2-$STAMP" ;;
    S3) python3 "$REPO/scripts/make-test-png.py" "$PNG" >/dev/null 2>&1 || { say "✗ 시험 PNG 생성 실패"; continue; }
        run_case S3-gtk-x11-image x11 image "$PNG" ;;
    *) say "모르는 시나리오: $c" ;;
  esac
done
say ""; say "### 결과: PASS=$PASS FAIL=$FAIL · 보고서 $REPORT · 조각 로그 $OUT/sync-$STAMP-*.log"
[ "$FAIL" -eq 0 ]
