# Homebrew Cask — Nexa Clip(설치본 채널: .dmg 안의 .app)
#
# 버전과 체크섬 자리는 릴리스 워크플로가 **실제 산출물 해시로** 채운다(render-manifests.sh).
# 참고 프로젝트(sosomlab-tauri-test1)의 기록에 "새 버전마다 cask의 version/sha256를
# 손으로 갱신해야 한다"가 마찰점으로 남아 있었다 — 손으로 적는 해시는 언젠가 틀리고,
# 틀린 해시는 사용자 기기에서 설치 실패로 나타난다. 그래서 여기서는 채우지 않는다.
#
# ★ **quarantine을 postflight에서 명시적으로 뗀다** — 그러지 않으면 앱이 실행되지 않는다.
#   실측(08-11, macOS 15 · Intel): 서명 없는 앱 + quarantine → **SIGKILL(exit 137)**,
#   그리고 macOS가 /Applications에서 앱을 치워 버린다. 애드혹 서명을 붙여도 결과는 같았다
#   (quarantine 자체가 원인 · 공증(notarization) 없이는 우회 불가).
#   → Homebrew Cask는 기본적으로 quarantine을 **붙인다**(브라우저 다운로드와 같게).
#     인증서를 갖추기 전까지는 여기서 떼는 것 외에 방법이 없다. 무엇을 왜 하는지
#     caveats에 그대로 밝힌다 — 사용자가 모르는 채로 보안 검사를 끄지는 않는다.
cask "nexa-clip" do
  arch arm: "arm64", intel: "x64"

  version "@VERSION@"
  sha256 arm:   "@SHA_MAC_ARM64_DMG@",
         intel: "@SHA_MAC_X64_DMG@"

  url "https://github.com/SosomLab/nexa-clip/releases/download/v#{version}/nexa-clip-#{version}-macos-#{arch}.dmg",
      verified: "github.com/SosomLab/nexa-clip/"
  name "Nexa Clip"
  desc "Cross-platform clipboard manager with encrypted history and E2E sync"
  homepage "https://github.com/SosomLab/nexa-clip"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :big_sur

  app "Nexa Clip.app"

  # 서명·공증이 없어 macOS가 실행을 막는다 — 설치 시점에 격리 표식을 뗀다.
  # (이 저장소에서 받은 것이 맞는지는 릴리스의 SHA256SUMS.txt로 확인할 수 있다.)
  #
  # ★ 낡은 손쉬운 사용 권한 항목도 함께 정리한다(09-07 사용자 실기 — brew upgrade 뒤 토글이 ON인데
  #   붙여넣기 불가 · 껐다 켜도 그대로 · 항목을 −로 지우고 다시 켜야 통함): TCC는 앱을 서명 요구사항으로
  #   기억하는데 애드혹 서명은 바이너리마다 요구사항(cdhash)이 달라 이전 항목이 새 앱과 맞지 않는다.
  #   업그레이드 시점에 항목을 지워 두면 첫 실행의 권한 대화상자가 새 항목을 만든다 → 사용자는 [켜기]만.
  #   sudo 불요 · 첫 설치(항목 없음)는 무해 · 실패해도 설치는 계속(must_succeed: false).
  #   앱도 시작 때 같은 정리를 한다(nclip-plat paste::warm_up — .dmg 직접 설치 대비).
  postflight do
    system_command "/usr/bin/tccutil",
                   args: ["reset", "Accessibility", "io.github.sosomlab.nexa-clip"],
                   sudo: false, must_succeed: false
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Nexa Clip.app"],
                   sudo: false
  end

  caveats <<~EOS
    이 앱은 코드 서명·공증이 되어 있지 않습니다(v1 · 인증서 미보유).
    설치 과정에서 macOS 격리 표식(com.apple.quarantine)을 제거해 바로 실행되도록 했습니다.

    붙여넣기(손쉬운 사용) 권한은 서명이 바뀌는 업그레이드마다 다시 켜야 합니다.
    설치 시 낡은 권한 항목을 정리해 두므로, 첫 실행의 권한 대화상자에서
    [시스템 설정 열기] → 손쉬운 사용 → Nexa Clip을 켜면 됩니다(항목 삭제 불필요).

    받은 파일이 맞는지 확인하려면 릴리스의 SHA256SUMS.txt와 대조하세요:
      https://github.com/SosomLab/nexa-clip/releases

    동기화를 켜면 같은 네트워크의 내 기기를 찾기 위해 로컬 네트워크 접근 권한을 요청합니다.
  EOS

  # 앱 데이터(설정·암호화 이력·동기화 신원 키·기기 목록)는 번들 밖에 둔다 — brew upgrade가 번들째 지워도 남게.
  zap trash: [
    "~/Library/Application Support/nexa-clip",
    "~/Library/Preferences/io.github.sosomlab.nexa-clip.plist",
    "~/Library/Saved Application State/io.github.sosomlab.nexa-clip.savedState",
  ]
end
