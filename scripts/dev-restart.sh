#!/usr/bin/env bash
# dev-restart.sh — Linux 개발 반복용: 빌드 → 기존 인스턴스 종료 → 트레이 재시작.
#
# 사용:  scripts/dev-restart.sh            # debug 빌드 + 재시작
#        scripts/dev-restart.sh --diag     # NEXA_CLIP_DIAG=1 로 감시 진단까지
# 로그:  target/debug/nexa-clip.log  (tail -f 로 살펴보기)
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p nexa-clip

# 기존 인스턴스 종료 — TERM → 5초 대기 → 잔류 시 KILL (Linux SIGINT 미이식 · 09-02 관찰)
OLD=$(pgrep -x nexa-clip || true)
if [ -n "$OLD" ]; then
  kill -TERM $OLD 2>/dev/null || true
  for p in $OLD; do timeout 5 tail --pid="$p" -f /dev/null || true; done
  pgrep -x nexa-clip >/dev/null && { kill -9 $OLD 2>/dev/null || true; sleep 0.3; }
  echo "종료: $OLD"
fi

LOG=target/debug/nexa-clip.log
ENVV=()
[ "${1:-}" = "--diag" ] && ENVV=(NEXA_CLIP_DIAG=1)
nohup env "${ENVV[@]}" ./target/debug/nexa-clip tray >"$LOG" 2>&1 &
NEW=$!
sleep 1
if kill -0 "$NEW" 2>/dev/null; then
  echo "재시작: pid $NEW · 로그 $LOG"
  head -12 "$LOG"
else
  echo "⚠️ 기동 실패 — 로그:"; cat "$LOG"; exit 1
fi
