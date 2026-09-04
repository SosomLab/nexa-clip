; Nexa Clip — Windows 설치본(NSIS)
;
; @VERSION@ / @SLUG@ 는 릴리스 워크플로가 치환한다.
;
; 설계 의도
; - **사용자 단위 설치**(HKCU · $LOCALAPPDATA) — 관리자 권한을 요구하지 않는다.
;   T0(무권한)에서 전 기능이 돌아야 한다는 원칙(DR-14)을 설치 단계에도 적용한다.
;   winget/choco의 무인 설치도 권한 상승 없이 통과한다.
; - `/S` **무인 설치**를 지원한다(winget·choco가 이걸 쓴다). 커스텀 입력 페이지를
;   두지 않아 무인 모드에서 물어볼 것이 없다.
; - 제거 정보를 레지스트리에 남긴다 — 없으면 winget이 설치 여부·버전을 알 수 없다.

Unicode true
!include "MUI2.nsh"

!define APPNAME    "Nexa Clip"
!define COMPANY    "SosomLab"
!define VERSION    "@VERSION@"
!define SLUG       "@SLUG@"
!define EXENAME    "nexa-clip.exe"
!define REGKEY     "Software\Microsoft\Windows\CurrentVersion\Uninstall\NexaClip"

Name "${APPNAME} ${VERSION}"
OutFile "nexa-clip-${VERSION}-${SLUG}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\NexaClip"
InstallDirRegKey HKCU "Software\${COMPANY}\NexaClip" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "0.0.0.0"
VIAddVersionKey "ProductName"     "${APPNAME}"
VIAddVersionKey "CompanyName"     "${COMPANY}"
VIAddVersionKey "FileDescription" "3-OS 동일 화면 클립보드 매니저"
VIAddVersionKey "FileVersion"     "${VERSION}"
VIAddVersionKey "ProductVersion"  "${VERSION}"
VIAddVersionKey "LegalCopyright"  "PolyForm Noncommercial 1.0.0"

!define MUI_ICON   "nexa-clip.ico"
!define MUI_UNICON "nexa-clip.ico"
!define MUI_ABORTWARNING
; 무인수 실행 = 트레이 상주(windows 서브시스템 · 콘솔 없음 · 09-03).
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXENAME}"
!define MUI_FINISHPAGE_RUN_PARAMETERS ""

!insertmacro MUI_PAGE_LICENSE "LICENSE.md"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "English"

Section "설치"
  SetOutPath "$INSTDIR"
  File "${EXENAME}"
  File "nclip-imgdec.exe" ; 이미지 격리 디코드 워커 — 없으면 PNG 썸네일·이미지 전파 변환이 빠진다
  File "nexa-clip.ico"
  File "README.md"
  File "LICENSE.md"

  ; 시작 메뉴 바로가기 = 트레이 상주.
  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\${EXENAME}" "" "$INSTDIR\nexa-clip.ico"

  WriteRegStr HKCU "Software\${COMPANY}\NexaClip" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\${COMPANY}\NexaClip" "Version"    "${VERSION}"

  ; winget/choco가 읽는 제거 정보. EstimatedSize는 KB.
  WriteRegStr   HKCU "${REGKEY}" "DisplayName"     "${APPNAME}"
  WriteRegStr   HKCU "${REGKEY}" "DisplayVersion"  "${VERSION}"
  WriteRegStr   HKCU "${REGKEY}" "Publisher"       "${COMPANY}"
  WriteRegStr   HKCU "${REGKEY}" "DisplayIcon"     "$INSTDIR\nexa-clip.ico"
  WriteRegStr   HKCU "${REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr   HKCU "${REGKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr   HKCU "${REGKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD HKCU "${REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${REGKEY}" "NoRepair" 1

  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  ; ⚠️ 사용자가 만든 것은 지우지 않는다 — 설치가 놓은 파일만 이름으로 지운다.
  ;    포터블 규약(실행 파일 옆 영속물)을 생각하면 RMDir /r는 위험하다.
  Delete "$INSTDIR\${EXENAME}"
  Delete "$INSTDIR\nclip-imgdec.exe"
  Delete "$INSTDIR\nexa-clip.ico"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE.md"
  Delete "$INSTDIR\uninstall.exe"
  RMDir  "$INSTDIR"                 ; 비어 있을 때만 지워진다

  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  DeleteRegKey HKCU "${REGKEY}"
  DeleteRegKey HKCU "Software\${COMPANY}\NexaClip"
SectionEnd
