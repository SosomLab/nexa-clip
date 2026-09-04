#!/usr/bin/env bash
# dev-install-mac.sh — mac 설치본 자리에서 실기(09-04 사용자 요청 · dev-install-win.ps1의 mac 판):
#   빌드 → 설치된 앱 번들(/Applications/Nexa Clip.app)에 실행 파일 2개 복사 → 기존 프로세스 종료 → 재시작.
#
# 사용:  scripts/dev-install-mac.sh            # 릴리스 프로필(기본 — 배포본과 같은 최적화)
#        scripts/dev-install-mac.sh --debug    # 디버그 프로필(패닉 위치 등 진단이 필요할 때)
# 전제:  설치본이 한 번은 설치돼 있어야 한다(brew install --cask nexa-clip 등 — 여기서는 바이너리만 바꾼다).
# 서명:  바이너리를 갈면 번들 서명 봉인이 깨진다 → 애드혹 재서명(배포본과 같은 adhoc · 공증 없음).
#        quarantine은 이미 없지만(cask postflight) 멱등으로 한 번 더 뗀다.
# 데이터: 설치본 데이터는 개발 인스턴스(target/*/data)와 **다른** 이력·설정·신원이다.
# 로그:  설치본은 콘솔이 없다 — 진단은 설정 → 고급 → 진단 로그(adv.log)로.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="/Applications/Nexa Clip.app"
DST="$APP/Contents/MacOS"

if [[ ! -d "$DST" ]]; then
    echo "설치 번들이 없습니다: $APP — 먼저 설치하세요(brew install --cask nexa-clip)" >&2
    exit 1
fi

PROFILE=release
FLAG=(--release)
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE=debug
    FLAG=()
fi

( cd "$ROOT" && cargo build "${FLAG[@]}" -p nexa-clip -p nclip-imgdec )

# 기존 프로세스 전부 종료 — 단일 인스턴스 가드가 있어 개발 인스턴스가 살아 있으면
# 설치본은 "열기"만 위임하고 끝난다(win 스크립트와 같은 이유). -x = 정확한 프로세스 이름.
for name in nexa-clip nclip-imgdec; do
    if pgrep -x "$name" >/dev/null; then
        echo "종료: $name ($(pgrep -x "$name" | tr '\n' ' '))"
        pkill -x "$name" || true
    fi
done
for _ in $(seq 1 20); do
    pgrep -x nexa-clip >/dev/null || break
    sleep 0.2
done

for f in nexa-clip nclip-imgdec; do
    src="$ROOT/target/$PROFILE/$f"
    cp -f "$src" "$DST/$f"
    printf '복사: %s (%s B)\n' "$f" "$(stat -f%z "$src")"
done

# 재서명 — 바이너리 교체로 깨진 번들 봉인을 다시 붙인다(Apple Silicon은 서명 필수).
# ★ 안정 신원(scripts/mac-dev-cert.sh 로 1회 생성)이 있으면 그것으로 — TCC(손쉬운 사용)가
#   서명으로 앱을 식별하므로 빌드를 갈아도 권한이 유지된다. 없으면 애드혹(교체마다 권한 리셋).
IDENTITY="${NEXA_SIGN_IDENTITY:-Nexa Clip Dev}"
if security find-identity -v -p codesigning 2>/dev/null | grep -q "$IDENTITY"; then
    codesign --force --deep --sign "$IDENTITY" "$APP" 2>/dev/null
    echo "서명: $IDENTITY (안정 신원 — 손쉬운 사용 권한 유지)"
else
    codesign --force --deep --sign - "$APP" 2>/dev/null
    echo "서명: 애드혹 — 교체마다 권한이 풀린다(scripts/mac-dev-cert.sh 로 안정 신원 1회 생성 권장)"
fi
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

VER="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
echo "실행: nexa-clip $VER ($PROFILE · $(uname -m)) · $APP"
open "$APP"    # 인자 없음 = 트레이 상주(설치본 규약 · 콘솔 없음)
sleep 2
pgrep -x nexa-clip >/dev/null \
    && ps -o pid=,lstart=,command= -p "$(pgrep -x nexa-clip | head -1)" \
    || { echo "★ 재시작 실패 — 수동 실행: open \"$APP\"" >&2; exit 1; }
# ⚠️ 붙여넣기 주입(⌘V)은 손쉬운 사용 권한 필요 — 애드혹 재서명이라 **바이너리를 갈 때마다
#    풀릴 수 있다**(TCC는 서명으로 앱을 식별). 주입이 안 되면: 시스템 설정 → 개인정보 보호
#    및 보안 → 손쉬운 사용 → Nexa Clip 체크 해제 후 다시 체크(없으면 + 로 추가).
echo "권한 확인: 주입(⌘V)이 안 붙으면 scripts/mac-grant-accessibility.sh (TCC 항목 재생성 · 토글은 옛 서명에 묶여 켜도 무효)"
