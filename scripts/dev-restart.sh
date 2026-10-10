#!/usr/bin/env bash
# dev-restart.sh — Linux 개발 반복용: 빌드 → 기존 Debug 인스턴스 종료 → 트레이 재시작.
#
# 사용:  scripts/dev-restart.sh            # debug 빌드 + 재시작
#        scripts/dev-restart.sh --diag     # NEXA_CLIP_DIAG=1 로 감시 진단까지
#        scripts/dev-restart.sh --all      # 설치본(/usr/bin 등 다른 경로)까지 종료
# 로그:  target/debug/nexa-clip.log  (tail -f 로 살펴보기)
#        직전 로그는 target/debug/nexa-clip.log.<YYYYMMDD-HHMMSS> 로 보관한다(덮어쓰지 않음 · 33 §5-5).
#
# ★ 10-10: 종전에는 `pgrep -x nexa-clip`으로 **실행 중 nexa-clip 전부**(apt 설치본 포함)를 종료했다.
#   이제 기본은 **이 트리의 target/debug/nexa-clip 으로 뜬 것만** 종료하고, 설치본은 `--all`에서만.
set -euo pipefail
cd "$(dirname "$0")/.."

DIAG=0; ALL=0
for a in "$@"; do
  case "$a" in
    --diag) DIAG=1 ;;
    --all)  ALL=1 ;;
    *) echo "모르는 인자: $a" >&2; exit 2 ;;
  esac
done

cargo build -p nexa-clip

BIN="$(realpath target/debug/nexa-clip)"

# 종료 대상 — 기본은 이 트리의 Debug 실행 파일로 뜬 프로세스만(빌드 뒤라 exe 링크에 " (deleted)"가 붙을 수 있다).
targets() {
  for p in $(pgrep -x nexa-clip || true); do
    if [ "$ALL" = 1 ]; then echo "$p"; continue; fi
    exe=$(readlink "/proc/$p/exe" 2>/dev/null || true)
    exe=${exe% (deleted)}
    [ "$exe" = "$BIN" ] && echo "$p" || true
  done
  return 0 # 대상이 없어도 성공 — 마지막 `[ ]`가 1을 돌려주면 set -e·pipefail이 스크립트를 끝낸다(10-10 첫 실검증)
}

# 기존 인스턴스 종료 — TERM → 5초 대기 → 잔류 시 KILL (Linux SIGINT 미이식 · 09-02 관찰)
OLD=$(targets | tr '\n' ' ' | sed 's/ *$//')
if [ -n "${OLD// /}" ]; then
  kill -TERM $OLD 2>/dev/null || true
  for p in $OLD; do timeout 5 tail --pid="$p" -f /dev/null || true; done
  for p in $OLD; do kill -0 "$p" 2>/dev/null && { kill -9 "$p" 2>/dev/null || true; }; done
  sleep 0.3
  echo "종료: $OLD"
fi
OTHERS=$(pgrep -x nexa-clip || true)
[ -n "$OTHERS" ] && echo "ℹ️ 다른 nexa-clip 실행 중(설치본 등 · 그대로 둠): $OTHERS — 함께 끄려면 --all"

LOG=target/debug/nexa-clip.log
if [ -s "$LOG" ]; then
  KEEP="$LOG.$(date +%Y%m%d-%H%M%S)"
  mv "$LOG" "$KEEP"
  [ -s "$LOG.err" ] && mv "$LOG.err" "$KEEP.err" || true
  echo "직전 로그 보관: $KEEP"
fi
ENVV=()
[ "$DIAG" = 1 ] && ENVV=(NEXA_CLIP_DIAG=1)
nohup env "${ENVV[@]}" ./target/debug/nexa-clip tray >"$LOG" 2>&1 &
NEW=$!
sleep 1
if kill -0 "$NEW" 2>/dev/null; then
  echo "재시작: pid $NEW · 로그 $LOG"
  head -12 "$LOG"
else
  echo "⚠️ 기동 실패 — 로그:"; cat "$LOG"; exit 1
fi
