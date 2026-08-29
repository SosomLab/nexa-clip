#!/usr/bin/env bash
# 리눅스 유형별 클립보드 실기 자동화 — T-14 Linux 1단(파일 복사/잘라내기 포함).
#
# 절차·환경은 docs/18 §9 · 점검표는 docs/21. 전제는 wl-clipboard + Wayland 세션이다.
# ⚠️ 이 스크립트는 **클립보드를 덮어쓴다** — 복사해 둔 것이 있으면 먼저 붙여 넣을 것.
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
printf '한글 텍스트 테스트' | wl-copy --type text/plain; settle

mark "2 HTML"
printf '<b>굵은 한글</b>' | wl-copy --type text/html; settle

mark "3 RTF"
printf '{\\rtf1\\ansi 리치텍스트}' | wl-copy --type text/rtf; settle

mark "4 PNG(256x256)"
wl-copy --type image/png < "$FIX/그림.png"; settle

mark "5 파일 1개(한글 이름) — 복사"
printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")" | wl-copy --type text/uri-list; settle

mark "6 파일 여러 개 — 복사"
{ printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")"
  printf 'file://%s\r\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/second file.txt")"
} | wl-copy --type text/uri-list; settle

mark "7 ★ 파일 잘라내기(GNOME x-special/gnome-copied-files)"
{ printf 'cut\n'
  printf 'file://%s\n' "$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$FIX/한글검증_v1.0.txt")"
} | wl-copy --type x-special/gnome-copied-files; settle

mark "8 민감 표식(KDE x-kde-passwordManagerHint=secret)"
printf 'secret' | wl-copy --type x-kde-passwordManagerHint; settle

echo "########## 4. 종료 ##########"
sleep 1; kill -TERM $WPID 2>/dev/null; sleep 1; kill -9 $WPID 2>/dev/null; wait $WPID 2>/dev/null
echo "########## 5. 결과 로그 ##########"
cat "$LOG"
