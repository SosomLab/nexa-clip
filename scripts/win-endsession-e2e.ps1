# win-endsession-e2e.ps1 — Windows 세션 종료(로그오프·시스템 종료) 경로 자동 시험(10-17 · T-75).
#   실제 로그오프 없이 **우리 Debug 인스턴스의 트레이 창(클래스 NexaClipTray)** 에만 WM_QUERYENDSESSION → WM_ENDSESSION(wParam=1)을
#   보내고, 앱이 저장을 마감하고 스스로 끝나는지 본다. 사용자 창·다른 프로세스·클립보드·키 입력에는 손대지 않는다.
#   ⚠️ 대상은 PID로만 집고, 기본은 **이 저장소 target\ 아래 exe**만 허용한다(설치본은 -AllowAny를 줘야 함 — 끝나면 상주가 멎는다).
#   사용:  pwsh scripts/win-endsession-e2e.ps1 -TargetPid 12345 [-LogPath target\debug\nexa-clip.log] [-TimeoutSec 10]
#   종료 코드: 0 = 통과 · 1 = 실패 · 2 = 인자/대상 오류.
param(
    [Parameter(Mandatory)][int]$TargetPid,
    [string]$LogPath = "",
    [int]$TimeoutSec = 10,
    [switch]$AllowAny
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

$proc = Get-Process -Id $TargetPid -ErrorAction SilentlyContinue
if (-not $proc) { Write-Host "대상 없음: PID $TargetPid 프로세스가 없습니다"; exit 2 }
if ($proc.ProcessName -ne 'nexa-clip') { Write-Host "대상 아님: PID $TargetPid = $($proc.ProcessName)"; exit 2 }
$exe = $proc.Path
$inTarget = $exe -and $exe.StartsWith((Join-Path $repo 'target'), [StringComparison]::OrdinalIgnoreCase)
if (-not $inTarget -and -not $AllowAny) {
    Write-Host "거부: $exe 는 이 저장소 target\ 아래가 아닙니다(설치본?) — 상주가 멎으므로 -AllowAny를 명시해야 합니다"
    exit 2
}

Add-Type -Namespace NexaE2E -Name EndSession -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hWnd, System.Text.StringBuilder sb, int max);
[DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern IntPtr SendMessageTimeoutW(IntPtr hWnd, uint msg, UIntPtr wParam, IntPtr lParam, uint flags, uint timeoutMs, out UIntPtr result);
public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
public static System.Collections.Generic.List<IntPtr> FindTrayWindows(uint pid) {
    var found = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, l) => {
        uint p; GetWindowThreadProcessId(h, out p);
        if (p != pid) return true;
        var sb = new System.Text.StringBuilder(256);
        GetClassNameW(h, sb, 256);
        if (sb.ToString() == "NexaClipTray") found.Add(h);
        return true;
    }, IntPtr.Zero);
    return found;
}
'@

$WM_QUERYENDSESSION = 0x0011
$WM_ENDSESSION = 0x0016
$SMTO_ABORTIFHUNG = 0x0002

$wins = [NexaE2E.EndSession]::FindTrayWindows([uint32]$TargetPid)
if ($wins.Count -eq 0) { Write-Host "실패: PID $TargetPid 에 NexaClipTray 창이 없습니다(트레이가 안 떴나?)"; exit 1 }
$hwnd = $wins[0]
Write-Host ("대상: PID {0} · {1} · NexaClipTray hwnd 0x{2:X}" -f $TargetPid, $exe, $hwnd.ToInt64())

$r = [UIntPtr]::Zero
$ok = [NexaE2E.EndSession]::SendMessageTimeoutW($hwnd, $WM_QUERYENDSESSION, [UIntPtr]::Zero, [IntPtr]::Zero, $SMTO_ABORTIFHUNG, 5000, [ref]$r)
if ($ok -eq [IntPtr]::Zero) { Write-Host "실패: WM_QUERYENDSESSION 응답 없음(5초)"; exit 1 }
Write-Host ("WM_QUERYENDSESSION → {0} (기대 1 = 종료 허용)" -f $r.ToUInt64())
if ($r.ToUInt64() -ne 1) { Write-Host "실패: 종료 허용(1)이 아님"; exit 1 }

$sw = [Diagnostics.Stopwatch]::StartNew()
$r = [UIntPtr]::Zero
# wParam = 1(세션이 정말 끝남) · lParam = 0(시스템 종료 · 로그오프면 0x80000000). 앱은 이 안에서 저장을 마감한다(상한 3초).
$ok = [NexaE2E.EndSession]::SendMessageTimeoutW($hwnd, $WM_ENDSESSION, [UIntPtr]::new(1), [IntPtr]::Zero, $SMTO_ABORTIFHUNG, 8000, [ref]$r)
$inMsg = $sw.ElapsedMilliseconds
if ($ok -eq [IntPtr]::Zero) { Write-Host "실패: WM_ENDSESSION 응답 없음(8초) — wndproc가 돌아오지 않음"; exit 1 }
Write-Host ("WM_ENDSESSION 처리 {0} ms (저장 마감 대기 포함 · 3000 ms 미만이면 신호로 깨어난 것)" -f $inMsg)

$exited = $proc.WaitForExit($TimeoutSec * 1000)
$total = $sw.ElapsedMilliseconds
if (-not $exited) { Write-Host ("실패: {0}초 안에 프로세스가 끝나지 않음(PID {1} 아직 실행 중)" -f $TimeoutSec, $TargetPid); exit 1 }
Write-Host ("프로세스 종료 ✓ · 신호 뒤 {0} ms · exit code {1}" -f $total, $proc.ExitCode)

$pass = $true
if ($LogPath -and (Test-Path $LogPath)) {
    $tail = Get-Content $LogPath -Tail 12
    $hitEnd = ($tail | Where-Object { $_ -match '세션 종료 신호' -and $_ -match '저장 마감 완료' } | Measure-Object).Count
    $hitBye = ($tail | Where-Object { $_ -match '종료합니다' } | Measure-Object).Count
    $hitTimeout = ($tail | Where-Object { $_ -match '저장 마감 대기 .*초과' } | Measure-Object).Count
    Write-Host ("로그: '세션 종료 신호 … 저장 마감 완료' {0}줄 · '종료합니다' {1}줄 · 대기 초과 {2}줄" -f $hitEnd, $hitBye, $hitTimeout)
    if ($hitEnd -lt 1 -or $hitTimeout -gt 0) { $pass = $false }
    Write-Host "--- 로그 꼬리 ---"
    $tail | ForEach-Object { Write-Host "  $_" }
} elseif ($LogPath) {
    Write-Host "로그 파일 없음: $LogPath (로그 판정 생략)"
}

if ($pass) { Write-Host "통과 — 세션 종료 경로: 허용 응답 · 저장 마감 · 자진 종료"; exit 0 }
Write-Host "실패 — 프로세스는 끝났지만 로그에 저장 마감 완료가 없음(또는 대기 초과)"; exit 1
