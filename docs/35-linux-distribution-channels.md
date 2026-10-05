# 35 · Linux 배포 채널 — 한 줄 설치 · 자동 업데이트 (조사 · 사용자 요청 10-05)

> **이 문서는**: winget·Chocolatey(Windows)·Homebrew(mac)처럼 **Linux에서 한 줄로 설치하고 자동으로 갱신**되는 배포 채널을 비교해, 사용자가 고를 수 있게 정리한 조사 문서다. **결정은 아직 없다**(→ [TODO](TODO.md) T-67).
> **표기**: **[확인]** = 1차 출처(공식 문서·정책)를 직접 읽음 · **[미확인]** = 기억 기반 · 출처 미확인 · **[저장소]** = 이 저장소·`nexa-beep`에서 직접 확인.
> **조사**: 2026-10-05 · 협업 세션(웹 조사는 하위 에이전트 · 링크는 그 시점 기준).

## 0. 결론 먼저

1. **라이선스 식별자는 표준이 있다** — SPDX에 `PolyForm-Noncommercial-1.0.0`이 정식 ID로 올라 있다 **[확인]**([spdx.org](https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0.json)). `LicenseRef-`를 쓸 필요가 없다.
2. **비자유(PolyForm NC) 때문에 막히는 곳**: Debian/Ubuntu 공식 저장소(main) · Fedora 공식·**COPR** · **Homebrew-core** · Cloudsmith 무료 OSS 플랜 · Launchpad PPA(Canonical 사전 승인 필요). **받아 주는 곳**: Flathub(조건부 — 소스 빌드) · Snap Store · AUR · nixpkgs(unfree) · 자체 tap · 자체 APT/RPM 저장소 · AppImage.
3. **샌드박스 채널(Flatpak·Snap)은 이 앱의 기능과 부딪친다** — 다른 앱 PID로 출처 앱 찾기(`/proc/<pid>/comm`) · `~/.config/autostart` 직접 쓰기 · X11 직접 접속이 걸린다. 들어가려면 **코드를 바꿔야** 한다(§3).
4. **가장 싸고 확실한 길 = 우리가 이미 만드는 `.deb`를 재사용** — 서명한 자체 APT(+RPM) 저장소를 GitHub Pages에 태그 CI로 발행 + 설치 스크립트. 사람 검수 없음 · 기능 제약 0.
5. **Homebrew는 이미 Linux에서 된다 [저장소]** — 우리 탭의 `nexa-clip-portable` formula에 `on_linux` 분기가 있다(`packaging/homebrew/nexa-clip-portable.rb`) → Linuxbrew 사용자는 지금도 `brew install` 가능.

## 1. 지금 상태 [저장소]

| 항목 | 값 |
|---|---|
| Linux 산출물 | GitHub Release에 `nexa-clip-<ver>-linux-x64.deb` + `…-linux-x64-portable.tar.gz`(`.github/workflows/release.yml` "Linux 설치본(.deb)" · "포터블 압축") · x86_64만 |
| 호환 범위 | zig 링커로 **glibc 2.17** 기준 링크 · 2.17 초과 심볼 게이트 · glibc 외 공유 라이브러리 링크 금지(Wayland·X11·xkbcommon은 런타임 dlopen) → 배포판 비종속 |
| `.deb` 의존 | `Depends: libc6 (>= 2.17)` · `Recommends: libx11-6, libxkbcommon0, libwayland-client0`(`packaging/linux/control`) |
| Homebrew | 탭 `kiros33/tap` — cask(mac) + formula `nexa-clip-portable`(mac + **`on_linux`** tar.gz) · 릴리스 때 자동 갱신 |
| 자동 갱신 | 없음(사용자가 새 `.deb`를 받아 설치) |
| nexa-beep 사례 | 같은 `.deb`·`tar.gz` + Homebrew 탭뿐 · `.rpm`·AppImage·Flatpak은 "수요 확인 후 결정"(beep TODO M5-4c · P3) — 참고할 선례 없음 |

### 앱이 Linux에서 쓰는 것(샌드박스 판정 기준)

| 기능 | 방식 |
|---|---|
| 클립보드 감시·게시 | X11/XWayland에 **직접 접속**(x11rb · XFIXES) — 포털 아님 |
| 전역 단축키 · 키 주입 | 포털 GlobalShortcuts · RemoteDesktop |
| 트레이 | SNI(D-Bus `org.kde.StatusNotifierWatcher`) |
| 자동 시작 · 런처 | `~/.config/autostart/nexa-clip.desktop` · `~/.local/share/applications` 직접 쓰기 |
| 동기화 | LAN 브로드캐스트 47301/udp + TCP 수락 · 릴레이 아웃바운드 47300 |
| 출처 앱 | `_NET_WM_PID` → **다른 프로세스의 `/proc/<pid>/comm`** |
| 설정 | gsettings(테마·키 반복 지연) · 포털 Settings |

## 2. 채널 비교 한 장

| 채널 | 사용자 설치 | 우리 제출 | 사람 검수 | CI 자동화 | 비자유 수용 | 기능 제약 | 갱신 |
|---|---|---|---|---|---|---|---|
| **자체 APT 저장소** | 키·저장소 등록 3줄(또는 스크립트) → `apt install` | 태그 CI가 서명·발행 | 없음 | ✅ 완전 | ✅(자체) | 없음 | `apt upgrade` |
| **자체 RPM 저장소** | `.repo` 1개 → `dnf install` | 태그 CI | 없음 | ✅ 완전 | ✅(자체) | 없음 | `dnf upgrade` |
| **설치 스크립트** | `curl -fsSL …/install.sh \| sh` | 스크립트 1개 | 없음 | ✅ | ✅ | 없음 | 저장소 등록 방식이면 패키지 관리자 |
| **AUR `-bin`** | `yay -S nexa-clip-bin` | 계정·SSH push | 사전 없음 | ✅(Action · 자기 책임) | ✅ | 없음 | AUR 헬퍼 |
| **Homebrew 탭** | `brew install kiros33/tap/nexa-clip-portable` | **이미 있음** | 없음 | ✅(이미) | ✅(자체 탭) | 없음 | `brew upgrade` |
| **AppImage** | 받아서 실행(+ Homebrew 6 Linux cask) | Release 자산 추가 · AppImageHub PR(선택) | Hub만 자동 시험 | ✅ | ✅(조항 없음) | 없음(경로 고정 주의) | zsync 델타(사용자 동의) |
| **Snap Store** | `sudo snap install nexa-clip` | 계정·이름 등록 | strict는 자동 · auto-connect 요청은 포럼 | ✅ | ✅ | **있음**(strict) | snapd 강제 자동 |
| **Flathub** | `flatpak install flathub …` | flathub PR | ✅ 자원봉사(수 주) | 검증 뒤 자동 병합 | 조건부(**소스 빌드**) | **있음**(코드 변경 필요) | `flatpak update` |
| **nixpkgs / flake** | `nix run github:…`(unfree 허용 필요) | flake 파일 또는 nixpkgs PR | flake 없음 · nixpkgs 있음 | ✅(flake) | unfree(캐시 없음) | 없음 | `nix flake update` |
| **Launchpad PPA** | `add-apt-repository ppa:…` | **소스만** 업로드 · LP 빌드 | — | 부분 | ❌ 사전 승인 필요 | 없음 | `apt` |
| **Fedora COPR** | `dnf copr enable …` | — | — | — | ❌ | — | — |
| **openSUSE OBS** | 배포판별 저장소 | `osc` | — | 가능 | ❓ 미확인 | 없음 | 패키지 관리자 |
| **cargo install / binstall** | `cargo binstall nexa-clip` | **crates.io 게시**(워크스페이스 크레이트 전부) | — | ✅ | ✅(SPDX ID) | 없음 | 없음(별도 도구) |

## 3. 채널별 상세

### 3-1. 자체 APT 저장소 (GitHub Pages · Cloudsmith · packagecloud)

- **대상**: Debian · Ubuntu · Mint · Pop!_OS 등 apt 계열 전부. 지금 `.deb`가 있어 **증분 비용이 가장 작다**.
- **사용자 설치**(예 · 주소는 가정):
  ```sh
  curl -fsSL https://sosomlab.github.io/apt/key.gpg | sudo tee /usr/share/keyrings/nexa-clip.gpg >/dev/null
  echo "deb [signed-by=/usr/share/keyrings/nexa-clip.gpg] https://sosomlab.github.io/apt stable main" \
    | sudo tee /etc/apt/sources.list.d/nexa-clip.list
  sudo apt update && sudo apt install nexa-clip
  ```
  Chrome·VS Code처럼 `.deb`의 postinst가 저장소·키를 스스로 등록하게 할 수도 있다(그러면 "`.deb` 한 번 설치 = 이후 자동 갱신").
- **발행**: `reprepro`·`aptly`(또는 `dpkg-scanpackages` + `apt-ftparchive`)로 색인 → GPG로 `InRelease` 서명 → Pages에 push. 태그 CI로 전부 자동화 · 사람 검수 없음.
- **규칙 [확인]**([Debian wiki — UseThirdParty](https://wiki.debian.org/DebianRepository/UseThirdParty)): *"A sources.list entry SHOULD have the signed-by option set. The signed-by entry MUST point to a file, and not a fingerprint."* · 패키지가 관리하는 키는 `/usr/share/keyrings` · 키 갱신은 `*-archive-keyring` 패키지로 권장 · deb822 `.sources` 형식 가능.
- **갱신**: `apt upgrade` · unattended-upgrades는 서드파티 origin을 따로 허용해야 한다.
- **라이선스**: 자체 호스팅이라 제약 없음. 단 **Cloudsmith 무료 OSS 플랜은 불가 [확인]**([정책](https://help.cloudsmith.io/docs/open-source-hosting-policy) — *"the primary project… must be free and open-source by definition"*) · packagecloud 무료 티어 조건 [미확인].
- **비용**: 서명 키 보관(GitHub Secret) · 키 만료 관리 · Pages 용량 한도(권장 약 1GB · 파일 100MB) [미확인].
- **Debian 공식(main) 불가 [확인]**([사회 계약 DFSG 6](https://www.debian.org/social_contract) — *"may not restrict the program from being used in a business"*). non-free 영역은 원칙상 가능하나 데비안 개발자 후원이 필요해 비현실적.

### 3-2. 자체 RPM 저장소 (Fedora · RHEL 계열 · openSUSE)

- **COPR 불가 [확인]**([COPR 사용자 문서](https://raw.githubusercontent.com/fedora-copr/copr/main/doc/user_documentation.rst) — *"…governed in whole or in part by a license not contained in the list of acceptable licenses for Fedora"*). Fedora 공식도 같은 이유로 불가.
- **대안**: `.rpm`을 만들어(`nfpm`·`cargo-generate-rpm` [미확인]) `createrepo_c`로 색인 · `rpm --addsign` + repomd 서명 → APT와 같은 Pages에 공존. 사용자는 `/etc/yum.repos.d/nexa-clip.repo`(`gpgcheck=1` · `gpgkey=URL`) → `dnf install nexa-clip`.
- 완전 자동화 · 검수 없음 · 기능 제약 없음. 지금 `.rpm` 산출물이 없어 **새로 만들어야** 한다(같은 바이너리라 포장만).

### 3-3. 설치 스크립트 (`curl | sh`)

- `curl -fsSL https://…/install.sh | sh` — 배포판을 감지해 **APT/RPM 저장소를 등록**(권장)하거나, 저장소가 없는 배포판은 `tar.gz`를 `~/.local/bin`에 푼다.
- 저장소 등록 방식이면 이후 갱신은 패키지 관리자 몫. tar 방식이면 갱신이 없다(재실행 또는 앱 안 갱신 확인 필요).
- 비용 최저 · 라이선스 제약 없음 · 신뢰를 위해 sha256/GPG 검증을 스크립트 안에서.

### 3-4. AUR (`nexa-clip-bin`)

- **대상**: Arch · Manjaro · EndeavourOS. 설치 `yay -S nexa-clip-bin`(또는 `paru`).
- **제출**: AUR 계정 + SSH 키로 git push · 사전 검수 없음. 미리 빌드한 산출물을 쓰면 `-bin` 접미사 필수 **[확인]**([AUR submission guidelines](https://wiki.archlinux.org/index.php?title=AUR_submission_guidelines) — *"Packages that use prebuilt deliverables, when the sources are available, must use the -bin suffix."*).
- **자동화**: 태그마다 `pkgver`·`sha256sums`를 갱신해 push하는 GitHub Action(예 `KSXGitHub/github-actions-deploy-aur`) [미확인] — 단 *"Automated PKGBUILD updates are used at your own risk and any malfunctioning accounts and their packages may be removed without prior notice."* **[확인]**.
- **라이선스**: 비자유 금지 조항 없음 · `license=('PolyForm-Noncommercial-1.0.0')` + LICENSE 파일 설치.
- 샌드박스 없음 → 기능 제약 없음.

### 3-5. Homebrew on Linux (Linuxbrew)

- **자체 탭 = 이미 된다 [저장소]**: `packaging/homebrew/nexa-clip-portable.rb`에 `on_linux do on_intel do url "…-linux-x64-portable.tar.gz"` 분기가 있다 → `brew install kiros33/tap/nexa-clip-portable`. 릴리스 때 자동 갱신(TAP_TOKEN)도 이미 돈다. 남은 일 = 위키·README 안내 정도.
- **Homebrew-core 불가 [확인]**([Acceptable Formulae](https://github.com/Homebrew/brew/blob/main/docs/Acceptable-Formulae.md) — *"must be open source under a licence compatible with the Debian Free Software Guidelines"*).
- **Linux cask [확인]**([Homebrew 6.0.0](https://brew.sh/2026/06/11/homebrew-6.0.0/)): 4.5(2025-04) 예비 지원 → 6.0(2026-06)에서 요건 명시 + **AppImage 지원** → cask가 더는 mac 전용이 아니다. 기존 cask에 Linux 분기를 넣는 길도 생겼다(문법 [미확인]).
- ⚠️ Homebrew의 Linux 기준 glibc가 2.39로 올라감 · Ubuntu 22.04는 Tier 2 **[확인]**(같은 글) — Linuxbrew 자체의 요구라 우리 바이너리(2.17)와는 별개지만, 오래된 배포판 사용자는 brew 경로를 못 쓸 수 있다.

### 3-6. AppImage (+ AppImageHub · zsync 갱신)

- 파일 하나 · 설치·root 불요 · **샌드박스 없음 → 기능 제약 없음**.
- **설치**: 받아서 `chmod +x` 후 실행 — "한 줄 설치" 개념은 약하다. Homebrew 6의 Linux cask가 AppImage를 받아 준다 **[확인]**.
- **AppImageHub**(appimage.github.io): `data/`에 GitHub 저장소 링크 한 줄 파일을 PR → Actions가 자동 시험 **[확인]**([README](https://github.com/AppImage/appimage.github.io/blob/master/README.md) — 가장 오래된 지원 Ubuntu LTS에서 실행 · X11 동작 · 30초 안에 창 · 영어 UI). 라이선스 조항은 없음.
- **갱신**: `appimagetool -u "gh-releases-zsync|SosomLab|nexa-clip|latest|*x86_64.AppImage.zsync"`로 갱신 정보를 넣고 `.zsync`를 Release에 함께 올리면 AppImageUpdate(또는 앱 안 libappimageupdate)가 델타 갱신 — 형식 문자열은 [미확인] · 원칙 *"Never download updates without the user's explicit consent"* **[확인]**([AppImage 문서](https://docs.appimage.org/packaging-guide/optional/updates.html)).
- ⚠️ 자동 시작·런처가 **AppImage 파일 경로**를 가리키게 되므로 파일을 옮기면 깨진다(`$APPIMAGE` 환경 변수로 경로를 잡아야 함 [미확인]).

### 3-7. Snap / Snap Store

- **대상**: Ubuntu 기본 탑재 · 다른 배포판은 snapd 설치. 설치 `sudo snap install nexa-clip`.
- **라이선스 수용 [확인]**([Snapcraft 블로그](https://snapcraft.io/blog/want-to-publish-a-snap-heres-a-list-of-dos-and-donts) — *"The Snap Store does not limit its developers to open-source or free software only."*) · 라이선스 필드는 SPDX 목록 기반.
- **classic 불가 [확인]**([classic 검수 기준](https://forum.snapcraft.io/t/process-for-reviewing-classic-confinement-snaps/1460) — 허용 = 컴파일러·디버거·IDE·터미널 등 · 거절 사유 = *"difficulty making strict confinement work"* · *"access to dot files in $HOME"*) → **strict**로 가야 한다.
- **strict에서의 기능**:
  - X11 직접 접속 = `x11` 인터페이스(auto-connect ✅ **[확인]**) · 포털 = `desktop` · 트레이 = `unity7`(auto-connect · SNI 용도는 [미확인]) · 네트워크 = `network`·`network-bind`.
  - ★ 출처 앱(`/proc/<pid>/comm`) = `system-observe` — **auto-connect 아님 [확인]**([인터페이스 문서](https://snapcraft.io/docs/reference/interfaces/system-observe-interface/)) → 사용자가 `snap connect`하거나 스토어에 auto-connect 요청(포럼 검수 · 수일~수 주 [미확인]).
  - ★ 자동 시작 = `~/.config/autostart` 직접 쓰기 불가 → `$SNAP_USER_DATA/.config/autostart/X.desktop` + `apps.<app>.autostart: X.desktop` 선언 **[확인]**([포럼](https://forum.snapcraft.io/t/how-to-add-autostart-to-the-app/8866)) → **코드 분기 필요**.
  - gsettings = `gsettings` 인터페이스 [미확인].
- **갱신**: snapd가 하루 여러 번 **강제** 자동 갱신 · 채널 stable/candidate/beta/edge. CI = `snapcore/action-build` + `action-publish`(`SNAPCRAFT_STORE_CREDENTIALS`) [미확인].
- Ubuntu 밖 사용자층의 반감 · 강제 갱신 정책은 감안할 것.

### 3-8. Flatpak / Flathub

- **대상**: 배포판 무관 · Fedora·Mint·Pop!_OS·SteamOS 기본 · Ubuntu는 따로 설치. 설치 `flatpak install flathub com.sosomlab.NexaClip`(ID는 가정).
- **라이선스 [확인]**([요건](https://raw.githubusercontent.com/flathub-infra/documentation/main/docs/02-for-app-authors/02-requirements.md)): 재배포 허용 + MetaInfo에 정확한 라이선스 — upstream이 직접 제출하면 재배포 허락은 암묵적. ★ 단 *"All source available submissions must be built entirely from source code."* · *"Binary or precompiled files must not be present in the submission pull request."* · 빌드 중 네트워크 금지 → **Release 바이너리를 감쌀 수 없다**. cargo 의존성을 `flatpak-cargo-generator`로 소스 목록화해 **오프라인 소스 빌드**해야 한다(zig 링커 빌드와 별도 경로). 신생 프로젝트는 *"a sustained commit history and standard release practices such as tagged versions"* 요구. MetaInfo `<project_license>PolyForm-Noncommercial-1.0.0</project_license>`.
- **검수 [확인]**([제출](https://raw.githubusercontent.com/flathub-infra/documentation/main/docs/02-for-app-authors/05-submission.md) — *"reviewers are volunteers and the response time may vary"* · *"There is no definite time limit"*) · 승인 뒤 `flathub/<appid>` 저장소 · External Data Checker가 갱신 PR 자동 · 검증 앱은 `automerge-flathubbot-prs` **[확인]**.
- **샌드박스와 이 앱 [확인 = 린터 규칙]**([린터](https://raw.githubusercontent.com/flathub-infra/documentation/main/docs/02-for-app-authors/12-linter.md)): *"When a suitable XDG portal exists… using the portal becomes mandatory."*
  - X11 직접 접속 = `--socket=x11` + `--share=ipc`(XWayland에도 붙음) — 클립보드 관리자 사유가 받아들여질지는 [미확인].
  - 트레이 = `--talk-name=org.kde.StatusNotifierWatcher`(흔히 허용 [미확인]) · 포털은 정상 · 네트워크 `--share=network`.
  - ★ 자동 시작 = `finish-args-autostart-filesystem-access` *"This exception is not granted as an Autostart portal exists."* **[확인]** → **Background 포털(RequestBackground autostart)로 코드 분기 필요**. `~/.local/share/applications` 접근도 린터 오류(런처는 Flatpak이 내보내므로 불필요).
  - ★ 출처 앱 = 샌드박스는 PID 네임스페이스가 따로라 호스트 PID(`_NET_WM_PID`)의 `/proc`을 못 읽는다 → **출처 앱 표시·제외 앱 게이트가 약해짐** [미확인].
  - gsettings = 샌드박스 안에서는 앱 자체 설정만 → 테마는 Settings 포털로(이미 포털 우선) [미확인].
- Flathub 웹은 OSI가 아닌 라이선스를 "Proprietary"로 표시할 가능성 [미확인].

### 3-9. Nix / nixpkgs · flake

- **nixpkgs [확인]**([설정 문서](https://github.com/NixOS/nixpkgs/blob/master/doc/using/configuration.chapter.md) — *"By default unfree software cannot be installed and doesn't show up in searches."* · *"Unfree software is not tested or built in Nixpkgs continuous integration, and therefore not cached."*) · `lib/licenses`에 PolyForm 항목 없음 → `unfreeRedistributable` 또는 사용자 정의.
- **자체 flake**: 저장소에 `flake.nix` → `nix run github:SosomLab/nexa-clip`(검수 없음 · 사용자는 allowUnfree 필요). prebuilt를 받아 `autoPatchelfHook`으로 감싸는 편이 현실적 [미확인].

### 3-10. Launchpad PPA

- **소스만 [확인]**([PPA 문서](https://ubuntu.com/docs/launchpad/user/reference/packaging/ppas/ppa/) — *"Source packages only; pre-built binary uploads are rejected."*) → Launchpad가 오프라인으로 빌드 · cargo vendor 동봉 · Ubuntu 버전별 rustc 문제 [미확인].
- **라이선스 [확인]**([Launchpad 약관](https://canonical.com/legal/launchpad-terms-of-service)) — OSI·FSF·DFSG 등 목록 밖은 *"bring it up on the Launchpad users mailing list for consideration before uploading it"* → **사전 승인 필요 · 불확실** · 비공개 PPA는 유료.

### 3-11. openSUSE OBS

- 한 프로젝트에서 RPM·Debian·Arch·AppImage·Flatpak을 여러 배포판용으로 빌드하고 서명 저장소·설치 안내를 자동 생성 **[확인]**([OBS 사용자 안내](https://openbuildservice.org/help/manuals/obs-user-guide/)).
- 빌드는 오프라인 · prebuilt tarball을 spec에서 설치하는 것은 기술적으로 가능 [미확인] · 공개 인스턴스 약관(오픈소스 전용 여부) 원문은 접근 실패 → [미확인]. 우리는 이미 `.deb`를 만들고 있어 이득이 적다.

### 3-12. cargo install / cargo-binstall

- crates.io `license` = SPDX 식 **[확인]**([Cargo manifest](https://doc.rust-lang.org/cargo/reference/manifest.html)) → `PolyForm-Noncommercial-1.0.0` 통과 예상(SPDX 등재 시점 [미확인]).
- 단 **워크스페이스 내부 크레이트(`nclip-*` 등)를 전부 crates.io에 게시**해야 하고, `cargo install`은 사용자 PC에서 소스 빌드(우리 zig·glibc 2.17 빌드와 다름). binstall은 Release 바이너리를 받지만 역시 crates.io 정보를 전제 **[확인]**([cargo-binstall](https://github.com/cargo-bins/cargo-binstall)). 자동 갱신 없음 → **비권장**.

### 3-13. 한 줄씩

- **Gentoo GURU**: 사용자 ebuild 저장소 · 기본 `ACCEPT_LICENSE="-* @FREE"`라 사용자가 라이선스 수락 필요 [미확인].
- **Void**: void-packages PR · 비자유는 restricted 취급 · 수용 까다로움 [미확인].
- **Alpine**: aports 메인테이너 검수 엄격 · 비자유 NC 사실상 어려움 [미확인].
- **Solus**: 공식 저장소 위주 · 비자유 제한적 [미확인].
- **Pacstall**(Ubuntu용 "AUR 비슷한" 저장소): `-bin` pacscript PR · 비자유 허용 여부 [미확인].
- **eget · ubi 등 GitHub Release 설치기**: 자산 이름 규칙만 맞으면 추가 작업 없이 동작 · 갱신은 재실행 [미확인].

### 3-14. Cloudflare Pages로 APT·RPM 저장소 호스팅 (사용자 질문 10-05 · 개발 세션 검토)

> 사용자는 홈페이지를 이미 **Cloudflare Pages**로 운영 중이다 → §3-1·§3-2의 호스팅(GitHub Pages 가정)을 Cloudflare Pages로 바꿀 수 있는지 검토했다. 아래 **[확인]** 은 개발 세션이 Cloudflare 공식 문서를 직접 읽은 것이다.

- **한도 [확인]**([Pages limits](https://developers.cloudflare.com/pages/platform/limits/)): 파일당 **25 MiB** · 파일 수 무료 **20,000** / 유료 100,000 · 빌드 무료 월 **500회** · `_redirects` 정적 **2,000** + 동적 **100**줄 · 줄당 1,000자 · 정적 자산 **대역폭 한도는 문서에 명시 없음**.
- **우리 산출물 크기 [저장소]**: v0.1.7 `.deb` 1,935,670 B · `tar.gz` 2,498,138 B → 25 MiB 한도에 넉넉하다.
- **갱신 반영 [확인]**([Serving Pages](https://developers.cloudflare.com/pages/configuration/serving-pages/)): 기본 헤더 `Cache-Control: public, max-age=0, must-revalidate` → 배포 즉시 반영. 단 *"adding caching to your custom domain may lead to stale assets being served after a deployment"* → ⚠️ **저장소 경로에는 zone 캐시 규칙을 걸지 않는다**(옛 색인과 새 색인이 섞이면 apt가 해시 불일치로 실패).
- **`_redirects` [확인]**: destination = *"A file path or external link"* · 상태 301/302/303/307/308(기본 302) · 스플랫·플레이스홀더 지원.
- **연계안 — 색인·서명·공개 키만 Pages, 패키지 파일은 GitHub Release**:
  - **APT**: `Packages`의 `Filename`은 저장소 기준 상대 경로만 쓸 수 있다 → `_redirects`로 `/apt/pool/<파일>` → `https://github.com/SosomLab/nexa-clip/releases/download/v<ver>/<파일>` **302**(CI가 버전마다 한 줄 생성).
  - **RPM**: `createrepo_c`의 기준 주소 옵션(`xml:base`)으로 Release 주소를 직접 적거나 [미확인 — 옵션 이름], APT와 같은 리다이렉트를 쓴다.
  - 무결성은 **서명된 색인의 해시·크기**가 지킨다 → 한 번 발행한 Release 자산은 **삭제·재업로드 금지**(해시가 바뀌면 그 버전 설치가 깨진다).
- **[미확인 · 착수 전 실기 필요]**: apt가 리다이렉트를 끝까지 따라가는지 — 특히 GitHub의 2단(release 주소 → objects 호스트). Ubuntu·Debian 실기로 확인한다. **안 되면 `.deb`를 Pages에 직접** 싣는다(크기는 한도 안).
- **권장 구성(개발 세션)**: 홈페이지와 **별도 Pages 프로젝트 + 하위 도메인**(예 `packages.sosomlab.com` — 주소는 가정). Pages 배포는 **사이트 전체 교체**라 CI가 매번 저장소 트리 전체를 생성해야 한다 → **최신 버전만 싣는** 구성이 단순하다.
- GitHub Packages는 apt/rpm 형식을 지원하지 않는다 [미확인].

### 3-15. winget식 "중앙 목록 등록" 후보 (개발 세션 검토 10-05 · 전부 [미확인])

winget-pkgs처럼 **중앙 목록 저장소에 PR 한 번** 넣으면 사용자가 그 도구로 설치·갱신하는 방식. 둘 다 **등록 조건·비자유(PolyForm NC) 수용 여부를 아직 조사하지 않았다**.

- **deb-get**: GitHub Release의 `.deb`를 가리키는 정의를 PR로 등록 · Ubuntu 계열 대상 [미확인].
- **AM / AppMan**: AppImage 설치 스크립트 목록에 PR로 등록 → §3-6 AppImage가 있어야 의미가 있다 [미확인].
- (같은 계열: §3-13 Pacstall.)

## 4. 권장 조합(결정은 사용자)

| 안 | 구성 | 작업량 | 도달 범위 | 위험 |
|---|---|---|---|---|
| **1안(권장)** | 서명 **APT + RPM 저장소**(GitHub Pages · 태그 CI 발행) + **설치 스크립트**(저장소 등록) + **AUR `-bin`** + 기존 **Homebrew 탭** 안내 | 중(서명 키·Pages 저장소·`.rpm` 포장·스크립트·AUR 계정) · 그 뒤 자동 | Debian/Ubuntu · Fedora/RHEL/openSUSE · Arch · Linuxbrew = 데스크톱 Linux 대부분 | 서명 키 관리(유출·만료) · AUR 자동 push는 자기 책임 · 사람 검수 없음 = 신뢰는 서명이 담보 |
| **2안(가벼움)** | 1안에서 RPM 빼고 **APT + 설치 스크립트**(+ 스크립트가 그 밖 배포판엔 tar.gz) + Homebrew 안내 | 소~중 | apt 계열 + 나머지는 수동 갱신 | Fedora·Arch는 자동 갱신 없음 |
| **3안(넓힘)** | 1안 + **AppImage(zsync)** · 이후 수요를 보고 **Snap(strict)** → **Flathub** | 대(Snap·Flatpak은 자동 시작 포털 분기 · 출처 앱 저하 대응 · Flathub은 오프라인 소스 빌드 경로 신설) | 앱스토어 노출(GNOME Software·Discover·Snap Store) | 검수 대기(Flathub 수 주) · 샌드박스로 기능 일부 저하 · Snap 강제 갱신 |

- **제외 권고**: Debian/Fedora 공식 · COPR · Homebrew-core · Cloudsmith OSS 플랜(라이선스) · Launchpad PPA(사전 승인 · 소스 업로드만) · cargo(crates.io 전부 게시 부담).
- **공통**: 모든 채널에 SPDX `PolyForm-Noncommercial-1.0.0` 표기 · winget·choco처럼 **제출물 영어 규칙**을 Linux 채널 메타데이터에도 적용할지 결정 필요(CLAUDE.md §4는 winget·choco만 명시).
- **추측 구분**: 위 표의 작업량·도달 범위는 협업 세션 추정이다. 정책 인용은 **[확인]** 표기한 것만 1차 출처.

## 5. 결정할 것(→ TODO T-67)

1. 채널 조합(1안·2안·3안 또는 다른 조합).
2. 서명 키 — 새로 만들지 · 보관 위치(GitHub Secret) · 만료 주기.
3. 저장소 호스팅 — `sosomlab.github.io` Pages 별도 저장소 vs 이 저장소 `gh-pages` vs **Cloudflare Pages 별도 프로젝트 + 하위 도메인**(§3-14 · 홈페이지와 같은 계정) · 패키지 파일을 Pages에 직접 둘지 · Release로 리다이렉트할지(apt 리다이렉트 실기가 먼저).
7. 중앙 목록 후보(deb-get · AM/AppMan · Pacstall)를 조사할지(§3-15).
4. Linux 채널 메타데이터 언어(영어 전용 규칙 확장 여부).
5. 샌드박스 채널을 갈 경우 코드 변경 범위(자동 시작 포털 분기 · 출처 앱 저하 수용).
6. nexa-beep과 같은 채널을 함께 쓸지(같은 서명 키·저장소 공유 여부 — 다른 저장소라 [22 전달 원장](22-upstream-beep-liaison.md) 대상 여부도).
