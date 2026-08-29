#!/usr/bin/env bash
# 리눅스 유형별 클립보드 실기 자동화 — T-14 Linux 1단(파일 복사/잘라내기 포함).
#
# 절차·환경은 docs/18 §9 · 점검표는 docs/21. 기본은 wl-clipboard + Wayland 세션이다.
# ⚠️ 이 스크립트는 **클립보드를 덮어쓴다** — 복사해 둔 것이 있으면 먼저 붙여 넣을 것.
#
# ★ X11 모드(08-29 추가) — `NCLIP_E2E_X11=1 bash scripts/linux-watch-e2e.sh`
#   `WAYLAND_DISPLAY`를 지우고 `xclip`으로 주입해 **`x11-xclip` 백엔드**를 탄다.
#   · Wayland 세션에서 돌리면 XWayland(`DISPLAY=:0`)의 X 클립보드를 본다 — Mutter가 양방향으로 잇는다.
#   · `NCLIP_E2E_XVFB=1`을 더하면 `Xvfb :99`를 띄워 **순수 X11 서버**에서 돈다(데스크톱 클립보드는 건드리지 않는다).
#   xclip은 셀렉션 주인으로 남기 위해 fork 해 상주한다 — 케이스마다 새 주인이 앞 것을 대체한다.
#
# ★ 로그 규율 둘(08-29에 실제로 데어서 넣었다):
#   ① `stdbuf -oL` — watch의 stdout이 파일이면 블록 버퍼링이라 종료 시 꼬리가 유실된다.
#   ② 케이스 마커를 감시 로그에 덧붙이지 않는다 — 두 writer의 오프셋이 충돌해 로그가 깨진다.
set -u
# 루트 없이 구성한 로컬 프리픽스가 있으면 쓴다(docs/18 §9-3).
[ -f "$HOME/.local/nclip-devenv/env.sh" ] && . "$HOME/.local/nclip-devenv/env.sh"
export PATH="$HOME/.cargo/bin:$PATH"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
SP="$(cd "$(dirname "$0")" && pwd)"
LOG="$SP/watch.log"
FIX="$SP/fixtures"
BIN="$REPO/target/debug/nexa-clip"

X11="${NCLIP_E2E_X11:-0}"
XVFB_PID=""
if [ "$X11" = 1 ]; then
  unset WAYLAND_DISPLAY
  if [ "${NCLIP_E2E_XVFB:-0}" = 1 ]; then
    Xvfb :99 -screen 0 640x480x24 -nolisten tcp >/dev/null 2>&1 & XVFB_PID=$!
    export DISPLAY=:99; sleep 1.5
  fi
  echo "### X11 모드 — DISPLAY=$DISPLAY (xclip 주입)"
fi
# 유형 하나를 클립보드에 쓴다: put <MIME> < 바이트. 백엔드에 따라 wl-copy / xclip.
put() {
  if [ "$X11" = 1 ]; then xclip -selection clipboard -t "$1" -i
  else wl-copy --type "$1"; fi
}

echo "########## 0. 빌드 ##########"
( cd "$REPO" && cargo build -p nexa-clip ) || { echo "빌드 실패"; exit 1; }

echo "########## 1. 고정물 준비 ##########"
rm -rf "$FIX"; mkdir -p "$FIX"
printf '한글 내용' > "$FIX/한글검증_v1.0.txt"
printf 'b' > "$FIX/second file.txt"
python3 "$REPO/scripts/make-test-png.py" "$FIX/그림.png"
ls -1 "$FIX"

echo "########## 2. capability ##########"
"$BIN" peek; echo "peek exit=$?"

echo "########## 3. watch 기동 ##########"
: > "$LOG"
NEXA_CLIP_DIAG=1 stdbuf -oL -eL "$BIN" watch > "$LOG" 2>&1 &
WPID=$!
sleep 2
if ! kill -0 $WPID 2>/dev/null; then echo "watch 즉시 종료 — 로그:"; cat "$LOG"; exit 1; fi

# 한 케이스 = 클립보드에 쓰고 잠깐 기다린다(폴링 500ms + 디바운스 500ms).
mark() { echo "===== CASE $1 ====="; }
settle() { sleep 2.5; }

mark "1 텍스트(한글)"
printf '한글 텍스트 테스트' | put text/plain; settle

mark "2 HTML"
printf '<b>굵은 한글</b>' | put text/html; settle

mark "3 RTF"
printf '{\\rtf1\\ansi 리치텍스트}' | put text/rtf; settle

mark "4 PNG(256x256)"
put image/png < "$FIX/그림.png"; settle

mark "5 파일 1개(한글 이름) — 복사"
printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")" | put text/uri-list; settle

mark "6 파일 여러 개 — 복사"
{ printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")"
  printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/second file.txt")"
} | put text/uri-list; settle

mark "7 ★ 파일 잘라내기(GNOME x-special/gnome-copied-files)"
{ printf 'cut\n'
  printf 'file://%s\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")"
} | put x-special/gnome-copied-files; settle

mark "8 민감 표식(KDE x-kde-passwordManagerHint=secret)"
printf 'secret' | put x-kde-passwordManagerHint; settle

echo "########## 4. 종료 ##########"
sleep 1; kill -TERM $WPID 2>/dev/null; sleep 1; kill -9 $WPID 2>/dev/null; wait $WPID 2>/dev/null
if [ "$X11" = 1 ]; then pkill -x xclip 2>/dev/null; [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null; fi
echo "########## 5. 결과 로그 ##########"
cat "$LOG"
