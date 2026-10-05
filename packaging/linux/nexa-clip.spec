# Nexa Clip RPM — release.yml이 이미 빌드한 실행 파일을 포장만 한다(.deb와 같은 내용물).
# 치환: @VERSION@(RPM 버전 — 사전 배포의 '-'는 '~') · 빌드 때 `--define "stage <내용물 폴더>"`.
# 패키지 서명은 하지 않는다 — pkg.sosomlab.com은 저장소 메타데이터 서명(repo_gpgcheck)으로 무결성을 지킨다.
Name:           nexa-clip
Version:        @VERSION@
Release:        1
Summary:        3-OS 동일 화면 클립보드 매니저
License:        PolyForm-Noncommercial-1.0.0
URL:            https://github.com/SosomLab/nexa-clip
Packager:       Sangyong Bae <kiros33@gmail.com>
Recommends:     libX11
Recommends:     libxkbcommon
Recommends:     libwayland-client

# 이미 만든 실행 파일을 그대로 싣는다 — 디버그 정보 분리·스트립·빌드 ID 링크를 하지 않는다.
%global debug_package %{nil}
%global __os_install_post %{nil}
%global _build_id_links none

%description
3-OS(Windows · macOS · Linux)에서 똑같이 생긴 화면으로 동작하는 클립보드 매니저입니다.
전부 Rust로 만든 단일 실행 파일이며, 자체 래스터라이저로 그려 Qt·WebView·Electron을 쓰지 않습니다.
이력은 로컬에 암호화 저장되고(기본 켜짐), 기기 사이 동기화는 승인한 기기와만 E2E로 이루어집니다
(같은 네트워크 직결 또는 릴레이 경유).

%install
mkdir -p %{buildroot}
cp -a %{stage}/. %{buildroot}/

%files
%attr(0755,root,root) /usr/bin/nexa-clip
%attr(0755,root,root) /usr/bin/nclip-imgdec
%attr(0644,root,root) /usr/share/applications/nexa-clip.desktop
%attr(0644,root,root) /usr/share/icons/hicolor/256x256/apps/nexa-clip.png
%dir %attr(0755,root,root) /usr/share/doc/nexa-clip
%attr(0644,root,root) /usr/share/doc/nexa-clip/README.md
%attr(0644,root,root) /usr/share/doc/nexa-clip/LICENSE.md
