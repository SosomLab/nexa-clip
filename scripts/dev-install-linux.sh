#!/usr/bin/env bash
# dev-install-linux.sh — Linux 설치본 자리에서 실기(09-05 사용자 요청 · dev-install-win.ps1·dev-install-mac.sh의 Linux 판):
#   빌드 → 설치된 자리(.deb = /usr/bin)에 실행 파일 2개 복사 → 기존 프로세스 종료 → 설치 런처로 재시작.
#
# 사용:  scripts/dev-install-linux.sh              # 릴리스 프로필(기본 — 배포본과 같은 최적화)
#        scripts/dev-install-linux.sh --debug      # 디버그 프로필(패닉 위치 등 진단이 필요할 때)
#        scripts/dev-install-linux.sh --assets     # 실행 파일 + .desktop·아이콘까지 갱신(패키징 파일을 고쳤을 때)
#        scripts/dev-install-linux.sh --no-start   # 복사만(재시작 안 함)
#        scripts/dev-install-linux.sh --deb        # ★ 최초 1회: 릴리스 .deb를 만들어 설치(sudo · 아래 참조)
# 전제:  설치본이 한 번은 설치돼 있어야 한다(mac=brew cask · win=setup.exe와 같은 자리 — 여기서는 바이너리만 바꾼다).
#        최초 설치: 배포된 .deb를 받아  sudo dpkg -i out/nexa-clip-<버전>-linux-x64.deb
#                   또는 이 스크립트의 --deb (로컬 소스로 .deb를 만들어 설치)
# 위치:  `command -v nexa-clip`로 **실제 설치 위치를 찾는다**(대개 /usr/bin). NEXA_INSTALL_DIR로 강제 지정 가능.
#        /usr 아래는 root 소유라 복사에 sudo가 붙는다(쓰기 가능하면 안 붙는다).
# 데이터: 설치본은 /usr(업그레이드 교체 자리)이라 exe 옆이 아니라 **~/.config/nexa-clip**을 쓴다 —
#        개발 인스턴스(target/debug/data)와 이력·설정·신원이 **다르다**(mac·win 스크립트와 같은 성질).
# 재시작: ★ 설치된 .desktop 런처로 띄운다(`gtk-launch nexa-clip`) — 바이너리를 직접 exec 하면 포털이 앱을
#        식별하지 못해 **전역 단축키 등록이 실패**한다(09-05 실측 · systemd-run·scope도 같은 이유로 실패).
# 로그:  설치본은 콘솔이 없다 — 런처 기동분은 journald로 간다: journalctl --user -f | grep nexa-clip
#        (또는 설정 → 고급 → 진단 로그 adv.log)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PROFILE=release
FLAG=(--release)
ASSETS=0
START=1
MAKE_DEB=0
for a in "$@"; do
    case "$a" in
        --debug)    PROFILE=debug; FLAG=() ;;
        --assets)   ASSETS=1 ;;
        --no-start) START=0 ;;
        --deb)      MAKE_DEB=1 ;;
        -h|--help)  sed -n '2,26p' "$0"; exit 0 ;;
        *)          echo "알 수 없는 옵션: $a (--help)" >&2; exit 2 ;;
    esac
done

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

# ── 권한 승격 도우미 — 대상이 쓰기 가능하면 안 쓴다(루트 없는 프리픽스 설치 지원).
#    ★ 터미널이 없는 자리(에디터 통합 셸·자동화)에서는 `sudo`가 "A terminal is required"로 죽는다 —
#    polkit 에이전트가 떠 있으면 `pkexec`(GUI 암호창)로 넘긴다. 둘 다 안 되면 그때 안내하고 멈춘다.
#    ⚠️ pkexec는 호출자의 작업 디렉터리를 물려주지 않는다 → 넘기는 경로는 **절대 경로**여야 한다.
ELEVATE=""
pick_elevator() {
    [ -n "$ELEVATE" ] && return 0
    if sudo -n true 2>/dev/null; then
        ELEVATE=sudo
    elif [ -t 0 ] && command -v sudo >/dev/null; then
        ELEVATE=sudo                       # 대화형 터미널 — 암호를 물어볼 수 있다
    elif command -v pkexec >/dev/null && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
        ELEVATE=pkexec                     # 데스크톱 세션 — polkit GUI 암호창
    elif command -v sudo >/dev/null; then
        ELEVATE=sudo
    else
        echo "권한 승격 수단이 없습니다(sudo·pkexec 부재) — NEXA_INSTALL_DIR로 쓰기 가능한 자리를 지정하세요" >&2
        exit 1
    fi
    [ "$ELEVATE" = pkexec ] && echo "권한: pkexec — 화면의 인증 창에 암호를 입력하세요"
}

run_as_owner() {
    local dir="$1"; shift
    if [ -w "$dir" ]; then
        "$@"
    else
        pick_elevator
        "$ELEVATE" "$@"
    fi
}

# ── --deb: 로컬 소스로 .deb를 만들어 설치(최초 1회 · release.yml의 Linux 설치본 단계와 같은 배치)
if [ "$MAKE_DEB" = 1 ]; then
    echo "── 릴리스 빌드(.deb용) ──"
    cargo build --release -p nexa-clip -p nclip-imgdec
    pkg="target/deb/nexa-clip_${VERSION}"
    rm -rf "$pkg"
    mkdir -p "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/share/applications" \
             "$pkg/usr/share/icons/hicolor/256x256/apps" "$pkg/usr/share/doc/nexa-clip"
    install -m 0755 target/release/nexa-clip    "$pkg/usr/bin/nexa-clip"
    install -m 0755 target/release/nclip-imgdec "$pkg/usr/bin/nclip-imgdec"
    install -m 0644 packaging/linux/nexa-clip.desktop "$pkg/usr/share/applications/nexa-clip.desktop"
    install -m 0644 packaging/branding/nexa-clip-256.png "$pkg/usr/share/icons/hicolor/256x256/apps/nexa-clip.png"
    install -m 0644 README.md LICENSE.md "$pkg/usr/share/doc/nexa-clip/"
    size_kb=$(du -sk "$pkg/usr" | cut -f1)
    sed -e "s/@VERSION@/$VERSION/g" -e "s/@SIZE@/$size_kb/g" packaging/linux/control > "$pkg/DEBIAN/control"
    mkdir -p out
    deb="out/nexa-clip-${VERSION}-linux-x64-local.deb"
    dpkg-deb --build --root-owner-group "$pkg" "$deb" >/dev/null
    echo "만듦: $deb ($(stat -c%s "$deb") B)"
    echo "설치: sudo dpkg -i $deb"
    sudo dpkg -i "$deb"
    echo "설치 완료 — 이후에는 옵션 없이 이 스크립트만 돌리면 된다(빠른 교체)."
    exit 0
fi

# ── 설치 위치 판정 — 실제 설치된 자리를 찾는다(PATH → 심볼릭 링크 해제 → 그 폴더).
if [ -n "${NEXA_INSTALL_DIR:-}" ]; then
    DST="$NEXA_INSTALL_DIR"
else
    INSTALLED="$(command -v nexa-clip || true)"
    if [ -z "$INSTALLED" ]; then
        cat >&2 <<'EOS'
설치본을 찾지 못했습니다(PATH에 nexa-clip 없음) — 먼저 한 번 설치하세요:
  ① 배포본:  curl -sLO https://github.com/SosomLab/nexa-clip/releases/latest/download/nexa-clip-<버전>-linux-x64.deb
              sudo dpkg -i nexa-clip-<버전>-linux-x64.deb
  ② 로컬 소스로:  scripts/dev-install-linux.sh --deb
  (비표준 위치에 설치했다면 NEXA_INSTALL_DIR=/그/폴더 로 지정)
EOS
        exit 1
    fi
    DST="$(dirname "$(readlink -f "$INSTALLED")")"
fi

# 개발 트리를 설치 위치로 오인하지 않는다(target/ 을 자기 자신에 덮어쓰는 사고 방지).
case "$DST" in
    "$ROOT"/target/*) echo "설치 위치가 개발 트리입니다($DST) — 배포본을 설치한 뒤 다시 실행하세요" >&2; exit 1 ;;
esac
[ -d "$DST" ] || { echo "설치 폴더가 없습니다: $DST" >&2; exit 1; }

echo "── 빌드($PROFILE) ──"
cargo build "${FLAG[@]}" -p nexa-clip -p nclip-imgdec

# ── 기존 프로세스 종료 — 단일 인스턴스 가드가 있어 살아 있으면 새로 띄운 설치본이 "열기"만 위임하고 끝난다.
#    (mac·win 스크립트와 같은 이유 · Linux는 SIGTERM → 정착 대기 → 잔류 시 KILL)
OLD="$(pgrep -x nexa-clip || true)"
if [ -n "$OLD" ]; then
    # shellcheck disable=SC2086
    kill -TERM $OLD 2>/dev/null || true
    for p in $OLD; do timeout 5 tail --pid="$p" -f /dev/null || true; done
    if pgrep -x nexa-clip >/dev/null; then
        # shellcheck disable=SC2086
        kill -9 $OLD 2>/dev/null || true
        sleep 0.3
    fi
    echo "종료: $(echo "$OLD" | tr '\n' ' ')"
fi
pkill -x nclip-imgdec 2>/dev/null || true

# ── 교체 — 실행 중이던 파일을 덮어쓰면 ETXTBSY가 날 수 있어 install(1)로 갈아 끼운다(원자적 rename).
for f in nexa-clip nclip-imgdec; do
    src="$ROOT/target/$PROFILE/$f"     # ★ 절대 경로 — pkexec는 호출자의 cwd를 물려주지 않는다
    [ -f "$src" ] || { echo "빌드 산출물이 없습니다: $src" >&2; exit 1; }
    run_as_owner "$DST" install -m 0755 "$src" "$DST/$f"
    printf '복사: %-14s %10d B → %s\n' "$f" "$(stat -c%s "$src")" "$DST/$f"
done

# ── 패키징 자산(선택) — .desktop·아이콘을 고쳤을 때만.
if [ "$ASSETS" = 1 ]; then
    share="$(dirname "$DST")/share"   # /usr/bin → /usr/share
    if [ -d "$share/applications" ]; then
        run_as_owner "$share/applications" install -m 0644 \
            "$ROOT/packaging/linux/nexa-clip.desktop" "$share/applications/nexa-clip.desktop"
        icon="$share/icons/hicolor/256x256/apps"
        run_as_owner "$icon" install -m 0644 "$ROOT/packaging/branding/nexa-clip-256.png" "$icon/nexa-clip.png"
        run_as_owner "$share/applications" update-desktop-database "$share/applications" 2>/dev/null || true
        echo "복사: .desktop · 아이콘 → $share"
    else
        echo "⚠️ 자산 위치를 못 찾음($share) — 실행 파일만 갈았습니다"
    fi
fi

[ "$START" = 1 ] || { echo "복사만 했습니다(--no-start)."; exit 0; }

# ── 재시작 — ★ 설치된 .desktop 런처로(포털 앱 식별 = 전역 단축키 등록 성공 · 09-05 실측).
#    바이너리를 직접 exec 하면 단축키가 "등록 실패"로 뜬다.
#    ⚠️ `gtk-launch nexa-clip`은 **ID로 찾으므로** 앱이 제 손으로 쓴 사용자 런처
#    (~/.local/share/applications/nexa-clip.desktop — 마지막에 띄운 실행 파일을 가리킨다)가
#    /usr/share 것을 가려 **개발 빌드가 대신 뜰 수 있다**. 그래서 설치된 경로를 **명시**한다.
#    로그: 설치본은 콘솔이 없다 — 자식이 물려받는 stdout을 파일로 돌려 **진단을 살린다**
#    (GNOME 자동 시작으로 뜬 인스턴스는 journald로 가지만, 여기서 띄운 것은 이 파일로 온다).
echo "── 재시작 ──"
DESKTOP="$(dirname "$DST")/share/applications/nexa-clip.desktop"
LOG="$ROOT/target/installed-nexa-clip.log"
mkdir -p "$(dirname "$LOG")"
: > "$LOG"
if command -v gio >/dev/null && [ -f "$DESKTOP" ]; then
    gio launch "$DESKTOP" >"$LOG" 2>&1 || true
elif command -v gtk-launch >/dev/null && [ -f "$DESKTOP" ]; then
    gtk-launch nexa-clip >"$LOG" 2>&1 || true
else
    echo "⚠️ 설치된 런처(.desktop)를 못 찾아 바이너리를 직접 띄웁니다 — 전역 단축키는 등록되지 않습니다"
    setsid "$DST/nexa-clip" >"$LOG" 2>&1 < /dev/null &
fi

for _ in $(seq 1 20); do
    pgrep -x nexa-clip >/dev/null && break
    sleep 0.3
done
NEW="$(pgrep -x nexa-clip | head -1 || true)"
if [ -z "$NEW" ]; then
    echo "★ 재시작 실패 — 수동: gtk-launch nexa-clip" >&2
    exit 1
fi
EXE="$(readlink -f "/proc/$NEW/exe")"
echo "실행: nexa-clip $VERSION ($PROFILE) · $EXE · pid $NEW"
# 데이터 자리 — /usr·/opt·brew keg·.app 번들은 "업그레이드 때 교체되는 자리"라 사용자 설정 폴더를 쓰고,
# 그 밖(포터블·임시 프리픽스)은 실행 파일 옆 data/ 를 쓴다(nexa_conf::is_replaced_on_upgrade).
case "$DST" in
    /usr/*|/opt/*|/snap/*|/nix/*|*/Cellar/*|*/homebrew/*|*/linuxbrew/*)
        echo "데이터: ${XDG_CONFIG_HOME:-$HOME/.config}/nexa-clip  (개발 인스턴스 target/*/data 와 별개)" ;;
    *)
        echo "데이터: $DST/data  (포터블 자리 = 실행 파일 옆 · 개발 인스턴스와 별개)" ;;
esac
echo "로그  : tail -f $LOG"
