# dev-install-win.ps1 — Windows 설치본 자리에서 실기(09-04 사용자 요청):
#   빌드 → setup으로 설치된 폴더(%LOCALAPPDATA%\Programs\NexaClip)에 실행 파일 2개 복사 → 기존 프로세스 종료 → 재시작.
#
# 사용:  pwsh scripts/dev-install-win.ps1            # 릴리스 프로필(기본 — 배포본과 같은 최적화)
#        pwsh scripts/dev-install-win.ps1 -Debug     # 디버그 프로필(패닉 위치 등 진단이 필요할 때)
# 전제:  설치본이 한 번은 설치돼 있어야 한다(폴더·바로가기·언인스톨러는 setup.exe 몫 — 여기서는 exe만 바꾼다).
# 데이터: 설치본은 exe 옆 data\ 를 쓴다 — 개발 인스턴스(target\debug\data)와 **다른** 이력·설정이다.
# 로그:  설치본은 콘솔이 없다 — 진단은 설정 → 고급 → 진단 로그(adv.log) 또는 `-Debug` + 콘솔 실행으로.

param([switch]$Debug)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dst = Join-Path $env:LOCALAPPDATA "Programs\NexaClip"
if (-not (Test-Path $dst)) {
    throw "설치 폴더가 없습니다: $dst — setup.exe로 먼저 설치하세요"
}
$profile = if ($Debug) { "debug" } else { "release" }

Push-Location $root
try {
    if ($Debug) { cargo build -p nexa-clip -p nclip-imgdec } else { cargo build --release -p nexa-clip -p nclip-imgdec }
    if ($LASTEXITCODE -ne 0) { throw "빌드 실패(exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}

# 기존 프로세스 전부 종료 — 단일 인스턴스 가드가 있어 개발 인스턴스가 살아 있으면 설치본은 "열기"만 위임하고 끝난다.
$running = Get-Process nexa-clip, nclip-imgdec -ErrorAction SilentlyContinue
if ($running) {
    $running | Stop-Process -Force
    Start-Sleep -Milliseconds 800
    Write-Host ("종료: " + (($running | ForEach-Object { "$($_.ProcessName)#$($_.Id)" }) -join " "))
}

foreach ($f in "nexa-clip.exe", "nclip-imgdec.exe") {
    $src = Join-Path $root "target\$profile\$f"
    Copy-Item $src (Join-Path $dst $f) -Force
    Write-Host ("복사: {0} ({1:N0} B)" -f $f, (Get-Item $src).Length)
}

$exe = Join-Path $dst "nexa-clip.exe"
$ver = & $exe --version
Write-Host "실행: $ver ($profile) · $dst"
Start-Process -FilePath $exe -WorkingDirectory $dst   # 인자 없음 = 트레이 상주(설치본 규약)
Start-Sleep -Seconds 2
Get-Process nexa-clip -ErrorAction SilentlyContinue | Select-Object Id, Path, StartTime | Format-Table -AutoSize
