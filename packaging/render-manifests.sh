#!/usr/bin/env bash
# 패키지 매니저 매니페스트 생성 — winget · Chocolatey · Homebrew
#
#   render-manifests.sh <VERSION> <자산디렉터리> <출력디렉터리>
#
# 자산 디렉터리에는 릴리스에 올라간 파일들이 그대로 있어야 한다(setup.exe·dmg·zip).
# 체크섬은 **그 파일들에서 직접 계산한다** — 손으로 적은 해시는 언젠가 틀리고,
# 틀린 해시는 사용자 기기에서 설치 실패로 나타난다.
#
# ★ 이 스크립트가 **유일한 치환 지점**이다. 릴리스 워크플로와 제출 워크플로가 각자
#   치환하면 언젠가 갈라지고, 갈라진 지점이 곧 버그가 된다(08-11에 같은 형태의
#   버그를 네 건 고쳤다 — 그리기와 이벤트가 다른 조건을 보고 있었다).
set -euo pipefail

VERSION="${1:?사용법: render-manifests.sh <VERSION> <자산디렉터리> <출력디렉터리>}"
ASSETS="${2:?자산 디렉터리}"
OUT="${3:?출력 디렉터리}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# sha256 — OS마다 도구 이름이 다르다(리눅스 sha256sum · macOS shasum).
sha() {
  local f="$ASSETS/$1"
  [ -f "$f" ] || { echo "::error::자산 없음: $1" >&2; exit 1; }
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | cut -d' ' -f1
  else
    shasum -a 256 "$f" | cut -d' ' -f1
  fi
}

V="$VERSION"
SHA_WIN_X64_SETUP=$(sha    "nexa-clip-$V-windows-x64-setup.exe")
SHA_WIN_ARM64_SETUP=$(sha  "nexa-clip-$V-windows-arm64-setup.exe")
SHA_WIN_X64_PORTABLE=$(sha "nexa-clip-$V-windows-x64-portable.zip")
SHA_WIN_ARM64_PORTABLE=$(sha "nexa-clip-$V-windows-arm64-portable.zip")
SHA_MAC_ARM64_DMG=$(sha    "nexa-clip-$V-macos-arm64.dmg")
SHA_MAC_X64_DMG=$(sha      "nexa-clip-$V-macos-x64.dmg")
SHA_MAC_ARM64_PORTABLE=$(sha "nexa-clip-$V-macos-arm64-portable.tar.gz")
SHA_MAC_X64_PORTABLE=$(sha "nexa-clip-$V-macos-x64-portable.tar.gz")
SHA_LINUX_X64_PORTABLE=$(sha "nexa-clip-$V-linux-x64-portable.tar.gz")
DATE=$(date -u +%Y-%m-%d)

fill() {
  sed -e "s/@VERSION@/$V/g" \
      -e "s/@DATE@/$DATE/g" \
      -e "s/@SHA_WIN_X64_SETUP@/$SHA_WIN_X64_SETUP/g" \
      -e "s/@SHA_WIN_ARM64_SETUP@/$SHA_WIN_ARM64_SETUP/g" \
      -e "s/@SHA_WIN_X64_PORTABLE@/$SHA_WIN_X64_PORTABLE/g" \
      -e "s/@SHA_WIN_ARM64_PORTABLE@/$SHA_WIN_ARM64_PORTABLE/g" \
      -e "s/@SHA_MAC_ARM64_DMG@/$SHA_MAC_ARM64_DMG/g" \
      -e "s/@SHA_MAC_X64_DMG@/$SHA_MAC_X64_DMG/g" \
      -e "s/@SHA_MAC_ARM64_PORTABLE@/$SHA_MAC_ARM64_PORTABLE/g" \
      -e "s/@SHA_MAC_X64_PORTABLE@/$SHA_MAC_X64_PORTABLE/g" \
      -e "s/@SHA_LINUX_X64_PORTABLE@/$SHA_LINUX_X64_PORTABLE/g" \
      "$1" > "$2"
}

mkdir -p "$OUT"

# ── winget(설치본 · 포터블) ──
# microsoft/winget-pkgs 경로 규약 = 소문자 첫 글자 / 식별자의 **점을 디렉터리로** 편다.
for ch in installer portable; do
  id="SosomLab.NexaClip"; [ "$ch" = portable ] && id="SosomLab.NexaClip.Portable"
  dir="$OUT/winget/manifests/s/$(echo "$id" | tr '.' '/')/$V"
  mkdir -p "$dir"
  fill "$here/winget/$ch/version.yaml"   "$dir/$id.yaml"
  fill "$here/winget/$ch/locale.yaml"    "$dir/$id.locale.en-US.yaml"
  fill "$here/winget/$ch/installer.yaml" "$dir/$id.installer.yaml"
done

# ── Chocolatey(설치본 · 포터블) — 그대로 `choco pack` 가능한 형태 ──
for ch in installer portable; do
  pkg="nexa-clip"; [ "$ch" = portable ] && pkg="nexa-clip-portable"
  dir="$OUT/choco/$pkg"
  mkdir -p "$dir/tools"
  fill "$here/choco/$ch/$pkg.nuspec"                     "$dir/$pkg.nuspec"
  fill "$here/choco/$ch/tools/chocolateyinstall.ps1"     "$dir/tools/chocolateyinstall.ps1"
  fill "$here/choco/$ch/tools/chocolateyuninstall.ps1"   "$dir/tools/chocolateyuninstall.ps1"

  # ★ Chocolatey 사람 검수 요구(0.1.5가 여기서 멈췄다 · 2026-09-28 — 자동 검사는 전부 통과했는데
  #   검수자가 "설명이 한국어뿐 · 비상업 제한이 영어로 안 읽힘 · <copyright> 없음"으로 돌려보냈다).
  #   자동 검사가 못 잡는 종류라 **여기서 멈춘다** — 제출 뒤에 알면 같은 버전을 다시 내야 한다.
  nus="$dir/$pkg.nuspec"
  grep -q '<copyright>[^<]\{1,\}</copyright>' "$nus" \
    || { echo "::error::$pkg.nuspec: <copyright>가 없다(Chocolatey 검수 요구)" >&2; exit 1; }
  grep -qi '^License: .*noncommercial use' "$nus" \
    || { echo "::error::$pkg.nuspec: 영어 라이선스 줄(License: … noncommercial use …)이 없다" >&2; exit 1; }
done

# 🔴 규칙(사용자 2026-10-04): **winget · Chocolatey에 제출하는 내용은 전부 영어로 쓴다** —
#   설명·요약뿐 아니라 스크립트 주석·출력 문구·YAML 주석까지(검수자와 스토어 사용자가 읽는다).
#   틀(packaging/winget · packaging/choco)에 영어가 아닌 글자가 들어가면 여기서 멈춘다.
#   판정 = ASCII 밖 바이트(파일 머리 BOM만 예외 — PowerShell 스크립트). 이 저장소 주석은 한국어지만
#   **제출되는 파일은 예외**다. Homebrew 탭은 우리 저장소라 대상이 아니다.
bad=0
while IFS= read -r f; do
  if hit=$(sed $'1s/^\xEF\xBB\xBF//' "$f" | LC_ALL=C grep -n $'[\x80-\xFF]'); then
    echo "$f:"; echo "$hit"; bad=1
  fi
done < <(find "$OUT/winget" "$OUT/choco" -type f \( -name '*.yaml' -o -name '*.nuspec' -o -name '*.ps1' \))
if [ "$bad" = 1 ]; then
  echo "::error::winget·choco 제출 파일에 영어가 아닌 글자가 있다(위 줄) — 전부 영어로 쓴다" >&2
  exit 1
fi

# ── Homebrew(탭 저장소 배치 그대로: Casks/ · Formula/) ──
mkdir -p "$OUT/homebrew/Casks" "$OUT/homebrew/Formula"
fill "$here/homebrew/nexa-clip.rb"          "$OUT/homebrew/Casks/nexa-clip.rb"
fill "$here/homebrew/nexa-clip-portable.rb" "$OUT/homebrew/Formula/nexa-clip-portable.rb"

# 남은 자리표시자가 있으면 **여기서 멈춘다** — 체크섬 자리에 @가 남은 매니페스트가
# 제출되면 설치가 깨진다. 조용히 넘어가면 안 되는 종류의 실수다.
if grep -rn '@[A-Z_]*@' "$OUT"; then
  echo "::error::치환되지 않은 자리표시자가 남았다" >&2
  exit 1
fi

echo "생성 완료 ($V):"
find "$OUT" -type f | sort
