# win-e2e.ps1 — Windows 자동 실기(10-10 사용자 "전체 자동화") · mac-paste-e2e.sh의 Windows 판.
#
# 무엇을 재나(전부 실제 제품 경로 · `--profile e2e`로 격리 — 데이터 폴더 data\profiles\e2e · 단일 인스턴스 가드 분리):
#   S1 기동     : tray 시작 → "클립보드 감시: ok"까지 시간 · 저장소 복원 줄 · 전역 단축키 줄
#   S2 캡처     : 실제 클립보드에 텍스트 3건 복사(Set-Clipboard) → 감시가 잡는다(검증은 S4에서 결과로)
#   S3 팝업     : 메모장을 앞에 두고 Shift+Alt+C(기본 퀵 팝업) → 팝업 창 등장 → Enter = 최근 항목 붙여넣기
#   S4 붙여넣기 : 메모장 본문에 마지막 복사 텍스트가 들어갔는가(Ctrl+A·Ctrl+C로 읽어 비교) + 로그 "키 주입 ok"
#   S5 설정 창  : `settings` 단독 실행 → 창 표시 시간 · 검색어 입력 · ★[닫기] 버튼 **마우스 클릭** → 프로세스 종료(P2-9 회귀)
#   S6 둘째 인스턴스: 같은 프로필 tray 재실행 = 열기 위임 뒤 즉시 종료
#   S7 메모리   : tray WS/Private(기동 직후 · 끝) · 설정 창 프로세스 WS
#
# 사용:  pwsh scripts/win-e2e.ps1 -Exe target\e2e\0.2.0\nexa-clip.exe     # 포터블(공식 zip 풀어 둔 것)
#        pwsh scripts/win-e2e.ps1                                           # 기본 = target\release\nexa-clip.exe
# 주의:  실제 클립보드를 쓴다(끝에 텍스트였으면 원복) · 키 주입(SendKeys)은 이 스크립트가 띄운 메모장·우리 창에만 ·
#        전역 단축키(Shift+Alt+C)는 **한 프로세스만** 등록할 수 있다 → 다른 nexa-clip(설치본·Debug)이 떠 있으면 팝업이
#        그쪽에 뜬다 — 돌리기 전에 끈다(감시 둘도 피한다).
# 결과:  콘솔 표 + target\e2e\result-<시각>.json · 로그 target\e2e\tray-<시각>.log

param(
    [string]$Exe = "",
    [string]$Profile = "e2e",
    [int]$ReadyTimeoutSec = 40
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
$root = Split-Path -Parent $PSScriptRoot
if (-not $Exe) { $Exe = Join-Path $root "target\release\nexa-clip.exe" }
$Exe = (Resolve-Path $Exe).Path
$outDir = Join-Path $root "target\e2e"
New-Item -ItemType Directory -Force $outDir | Out-Null
$stamp = Get-Date -Format "MMdd-HHmmss"
$trayLog = Join-Path $outDir "tray-$stamp.log"
$trayErr = Join-Path $outDir "tray-$stamp.err"
$results = [ordered]@{}
$metrics = [ordered]@{}

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
using System.Collections.Generic;
public static class W32 {
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    public static List<Tuple<IntPtr,string,uint>> Visible() {
        var list = new List<Tuple<IntPtr,string,uint>>();
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) return true;
            var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
            if (sb.Length == 0) return true;
            uint pid; GetWindowThreadProcessId(h, out pid);
            list.Add(Tuple.Create(h, sb.ToString(), pid));
            return true;
        }, IntPtr.Zero);
        return list;
    }
    public static string FgTitle() {
        var sb = new StringBuilder(512); GetWindowTextW(GetForegroundWindow(), sb, 512); return sb.ToString();
    }
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(60);
        mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero); // LEFTDOWN
        System.Threading.Thread.Sleep(60);
        mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero); // LEFTUP
    }
}
"@

# 제목이 패턴 중 하나와 맞는 보이는 창(최대 WaitSec 동안 150ms 간격으로 다시 본다).
function Find-Window([string[]]$Patterns, [int]$WaitSec = 5) {
    $deadline = (Get-Date).AddSeconds($WaitSec)
    do {
        foreach ($w in [W32]::Visible()) {
            foreach ($p in $Patterns) { if ($w.Item2 -like $p) { return $w } }
        }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}
function Wait-LogLine([string]$Path, [string]$Pattern, [int]$TimeoutSec) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        if ((Test-Path $Path) -and (Select-String -Path $Path -Pattern $Pattern -Quiet -ErrorAction SilentlyContinue)) { return $true }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    return $false
}
function Log-Line([string]$Path, [string]$Pattern) {
    $m = Select-String -Path $Path -Pattern $Pattern -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($m) { return $m.Line } else { return "" }
}
function Mem([int]$ProcId) {
    $p = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
    if (-not $p) { return $null }
    return [ordered]@{ ws_mb = [math]::Round($p.WorkingSet64 / 1MB, 1); private_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); peak_mb = [math]::Round($p.PeakWorkingSet64 / 1MB, 1) }
}
function Mark([string]$Step, [bool]$Ok, [string]$Note) {
    $results[$Step] = [ordered]@{ ok = $Ok; note = $Note }
    $tag = if ($Ok) { "PASS" } else { "FAIL" }
    Write-Host ("[{0}] {1} — {2}" -f $tag, $Step, $Note)
}
function Keys([string]$s) { [System.Windows.Forms.SendKeys]::SendWait($s) }
function Read-Notepad($hwnd) {
    [void][W32]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 400
    Keys "^a"; Start-Sleep -Milliseconds 150; Keys "^c"; Start-Sleep -Milliseconds 500
    try { return (Get-Clipboard -Raw) } catch { return "" }
}
# ★ 사용한 메모장 전부 닫기(10-10 사용자 규칙 "테스트에 쓴 노트패드는 모두 닫히게"). Win11 메모장은 기존 프로세스에 **탭**으로 열려
#   띄운 PID를 죽여도 창이 남고, 미저장 본문은 다음 실행에 복원된다 → 본문을 비우고(Ctrl+A·Del) 탭을 닫아(Ctrl+W · 저장 질문 없음)
#   그래도 남은 창은 그 프로세스를 종료한다. 대상 = 제목에 우리 표식(e2e)이 있는 메모장 창 + 이 실행 중 시작된 Notepad 프로세스.
function Close-Notepads([datetime]$Since) {
    foreach ($w in [W32]::Visible()) {
        $isNp = ($w.Item2 -like "*메모장*" -or $w.Item2 -like "*Notepad*")
        if (-not $isNp -or $w.Item2 -notlike "*e2e*") { continue }
        [void][W32]::SetForegroundWindow($w.Item1); Start-Sleep -Milliseconds 300
        Keys "^a"; Start-Sleep -Milliseconds 120; Keys "{DEL}"; Start-Sleep -Milliseconds 200
        Keys "^w"; Start-Sleep -Milliseconds 500
    }
    Start-Sleep -Milliseconds 300
    foreach ($w in [W32]::Visible()) {
        if (($w.Item2 -like "*메모장*" -or $w.Item2 -like "*Notepad*") -and $w.Item2 -like "*e2e*") {
            Stop-Process -Id $w.Item3 -Force -ErrorAction SilentlyContinue
        }
    }
    Get-Process Notepad -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -gt $Since } |
        ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
}

# ── 전제: 같은 프로필 인스턴스가 남아 있으면 정리(다른 프로필·설치본은 건드리지 않는다) ──
Get-CimInstance Win32_Process -Filter "Name='nexa-clip.exe'" | Where-Object { $_.CommandLine -like "*--profile $Profile*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
$others = @(Get-Process nexa-clip -ErrorAction SilentlyContinue)
if ($others.Count -gt 0) { Write-Host ("경고: 다른 nexa-clip {0}개가 떠 있음(PID {1}) — 전역 단축키를 그쪽이 쥐면 S3가 실패한다" -f $others.Count, ($others.Id -join ",")) }
$origClip = $null
try { $origClip = Get-Clipboard -Raw -ErrorAction SilentlyContinue } catch {}
$notepad = $null; $tray = $null; $settings = $null
$runStart = Get-Date

try {
    # ── S1 기동 ──
    $t0 = Get-Date
    $tray = Start-Process -FilePath $Exe -ArgumentList "--profile", $Profile, "tray" -PassThru -RedirectStandardOutput $trayLog -RedirectStandardError $trayErr -WindowStyle Hidden
    $ready = Wait-LogLine $trayLog "클립보드 감시: ok" $ReadyTimeoutSec
    $tReady = ((Get-Date) - $t0).TotalSeconds
    Start-Sleep -Milliseconds 800 # 단축키 등록 줄은 감시 ok 뒤에 찍힌다
    $restore = Log-Line $trayLog "저장소: "
    $hotkey = Log-Line $trayLog "전역 단축키: "
    $metrics.start_to_ready_sec = [math]::Round($tReady, 2)
    $metrics.restore_line = $restore
    $metrics.hotkey_line = $hotkey
    Mark "S1 기동" $ready ("{0:N2}s · {1} · {2}" -f $tReady, $restore, $hotkey)
    if (-not $ready) { throw "기동 실패 — $trayErr 확인" }
    Start-Sleep -Seconds 1
    $metrics.mem_tray_ready = Mem $tray.Id

    # ── S2 캡처 — 실제 클립보드에 3건 ──
    $marker = "E2E-$stamp"
    foreach ($i in 1..3) { Set-Clipboard -Value "$marker-$i"; Start-Sleep -Milliseconds 800 }
    Start-Sleep -Seconds 2
    Mark "S2 캡처(복사 3건)" $true "$marker-1..3 복사 완료(검증은 S4 결과로)"

    # ── S3 팝업 — 메모장 앞에 두고 Shift+Alt+C → Enter ──
    $notepad = Start-Process notepad.exe -PassThru
    Start-Sleep -Seconds 2
    $np = Find-Window @("*메모장*", "*Notepad*") 6
    if (-not $np) { throw "메모장 창을 못 찾음" }
    [void][W32]::SetForegroundWindow($np.Item1)
    Start-Sleep -Milliseconds 600
    Keys "e2e: "
    Start-Sleep -Milliseconds 600
    $metrics.fg_before_hotkey = [W32]::FgTitle()
    $tPop0 = Get-Date
    Keys "+%c" # 기본 Shift+Alt+C(설정이 다르면 이 줄을 맞춘다)
    $popup = Find-Window @("Nexa Clip*[$Profile]*", "Nexa Clip*") 5
    $tPop = ((Get-Date) - $tPop0).TotalSeconds
    $metrics.popup_show_sec = [math]::Round($tPop, 2)
    Mark "S3 팝업(Shift+Alt+C)" ($null -ne $popup) ("창 '{0}' · {1:N2}s · 직전 포그라운드 '{2}'" -f $popup.Item2, $tPop, $metrics.fg_before_hotkey)
    if ($popup) {
        Start-Sleep -Milliseconds 800
        Keys "{ENTER}"
        Start-Sleep -Milliseconds 300
        $metrics.fg_after_enter = [W32]::FgTitle()
    }
    $pasted = Wait-LogLine $trayLog "키 주입 ok" 6
    Start-Sleep -Milliseconds 2500 # 주입 뒤 대상 앱이 처리할 시간(10-10 1차 E2E: 1초는 짧았다)

    # ── S4 붙여넣기 검증 — 메모장 본문 읽기 ──
    try { $metrics.clip_after_paste = Get-Clipboard -Raw } catch { $metrics.clip_after_paste = "" }
    $body = Read-Notepad $np.Item1
    $okPaste = $pasted -and ($body -like "*$marker-3*")
    Mark "S4 붙여넣기(최근 항목 → 메모장)" $okPaste ("로그 주입 ok={0} · 본문='{1}' · Enter 뒤 포그라운드 '{2}' · 클립보드 '{3}'" -f $pasted, ($body -replace "`r?`n", "⏎"), $metrics.fg_after_enter, $metrics.clip_after_paste)

    # ── S5 설정 창 — 열기 시간 · 검색 입력 · [닫기] 클릭 → 종료 ──
    $tSet0 = Get-Date
    $settings = Start-Process -FilePath $Exe -ArgumentList "--profile", $Profile, "settings" -PassThru -WindowStyle Normal
    $sw = Find-Window @("Nexa Clip*Settings*", "Nexa Clip*설정*") 15
    $tSet = ((Get-Date) - $tSet0).TotalSeconds
    $metrics.settings_show_sec = [math]::Round($tSet, 2)
    Mark "S5a 설정 창 표시" ($null -ne $sw) ("'{0}' · {1:N2}s" -f $sw.Item2, $tSet)
    if ($sw) {
        Start-Sleep -Milliseconds 800
        $metrics.mem_settings = Mem $settings.Id
        [void][W32]::SetForegroundWindow($sw.Item1)
        Start-Sleep -Milliseconds 300
        Keys "theme"
        Start-Sleep -Milliseconds 800
        # [닫기] = 하단 줄 오른쪽 끝(settings.rs layout: 우측 여백 PAD 12 · 폭 90 · 하단 줄 44 — 논리 px × DPI 배율).
        $cr = New-Object W32+RECT; [void][W32]::GetClientRect($sw.Item1, [ref]$cr)
        $pt = New-Object W32+POINT; $pt.X = 0; $pt.Y = 0; [void][W32]::ClientToScreen($sw.Item1, [ref]$pt)
        $dpi = [W32]::GetDpiForWindow($sw.Item1); if ($dpi -eq 0) { $dpi = 96 }
        $s = $dpi / 96.0
        $cx = [int]($pt.X + $cr.R - (12 + 45) * $s)
        $cy = [int]($pt.Y + $cr.B - 22 * $s)
        [W32]::Click($cx, $cy)
        $exited = $settings.WaitForExit(4000)
        if (-not $exited) {
            [void][W32]::SetForegroundWindow($sw.Item1); Keys "{ESC}"
            $escExited = $settings.WaitForExit(3000)
            Mark "S5b [닫기] 클릭 → 종료" $false ("클릭({0},{1}) 뒤 4초 안 종료 안 함 · Esc 종료={2}" -f $cx, $cy, $escExited)
        } else {
            Mark "S5b [닫기] 클릭 → 종료" $true ("클릭({0},{1}) · DPI {2}" -f $cx, $cy, $dpi)
        }
    }

    # ── S6 둘째 인스턴스 = 열기 위임 뒤 종료 ──
    $second = Start-Process -FilePath $Exe -ArgumentList "--profile", $Profile, "tray" -PassThru -WindowStyle Hidden
    $secondExited = $second.WaitForExit(8000)
    Mark "S6 둘째 인스턴스(열기 위임)" $secondExited ("8초 안 종료={0}" -f $secondExited)
    $mw = Find-Window @("Nexa Clip*[$Profile]*") 3
    if ($mw) { [void][W32]::SetForegroundWindow($mw.Item1); Start-Sleep -Milliseconds 300; Keys "{ESC}" }

    # ── S7 메모리·오류 ──
    Start-Sleep -Seconds 2
    $metrics.mem_tray_end = Mem $tray.Id
    $errLines = @(); if (Test-Path $trayErr) { $errLines = @(Get-Content $trayErr | Where-Object { $_.Trim() }) }
    $panic = @(Select-String -Path $trayLog, $trayErr -Pattern "panicked|RUST_BACKTRACE" -ErrorAction SilentlyContinue)
    Mark "S7 오류·패닉 0" (($errLines.Count -eq 0) -and ($panic.Count -eq 0)) ("stderr {0}줄 · panic {1}" -f $errLines.Count, $panic.Count)
    $metrics.tray_alive_at_end = -not $tray.HasExited
}
finally {
    if ($settings -and -not $settings.HasExited) { Stop-Process -Id $settings.Id -Force -ErrorAction SilentlyContinue }
    # 메모장 정리(규칙 10-10) — 띄운 PID만이 아니라 우리가 쓴 창 전부(본문 비우기 → 탭 닫기 → 남으면 프로세스 종료).
    try { Close-Notepads $runStart } catch {}
    if ($notepad -and -not $notepad.HasExited) { Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue }
    if ($tray -and -not $tray.HasExited) { Stop-Process -Id $tray.Id -Force -ErrorAction SilentlyContinue }
    if ($null -ne $origClip -and $origClip -is [string]) { try { Set-Clipboard -Value $origClip } catch {} }
}

$pass = @($results.Values | Where-Object { $_.ok }).Count
$total = $results.Count
Write-Host ""
Write-Host ("결과: {0}/{1} PASS · exe {2}" -f $pass, $total, $Exe)
Write-Host ("기동 {0}s · 팝업 {1}s · 설정 창 {2}s · tray WS {3}→{4} MB(peak {5}) · settings WS {6} MB" -f $metrics.start_to_ready_sec, $metrics.popup_show_sec, $metrics.settings_show_sec, $metrics.mem_tray_ready.ws_mb, $metrics.mem_tray_end.ws_mb, $metrics.mem_tray_end.peak_mb, $metrics.mem_settings.ws_mb)
$json = Join-Path $outDir "result-$stamp.json"
[ordered]@{ exe = $Exe; profile = $Profile; results = $results; metrics = $metrics; log = $trayLog } | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 $json
Write-Host "JSON: $json · 로그: $trayLog"
if ($pass -ne $total) { exit 1 }
