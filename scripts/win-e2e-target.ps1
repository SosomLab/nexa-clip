# win-e2e-target.ps1 — E2E 붙여넣기 **대상 창**(10-10 사고 뒤 · 메모장 대체). win-e2e.ps1이 띄우는 스크립트 전용 편집 창.
#   WinForms 멀티라인 TextBox 하나 · 제목 = 인자 · 뜨면 HwndFile에 "HWND⏎PID" 기록 · 200ms마다 본문을 TextFile에 덤프.
#   읽기 = 파일(Ctrl+A·Ctrl+C 없음) · 닫기 = 부모가 이 프로세스를 끝낸다(자동 저장·탭 복원 없음) → 사용자 문서·프로세스를 건드릴 길이 없다.
#   단독 확인:  pwsh scripts/win-e2e-target.ps1 target\e2e\t.hwnd target\e2e\t.txt "e2e target"
param(
    [Parameter(Mandatory)][string]$HwndFile,
    [Parameter(Mandatory)][string]$TextFile,
    [string]$Title = "nexa-clip e2e target"
)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$form = New-Object System.Windows.Forms.Form
$form.Text = $Title
$form.Width = 640
$form.Height = 420
$form.StartPosition = 'CenterScreen'
$tb = New-Object System.Windows.Forms.TextBox
$tb.Multiline = $true
$tb.Dock = 'Fill'
$tb.ScrollBars = 'Vertical'
$tb.AcceptsReturn = $true
$tb.Font = New-Object System.Drawing.Font('Consolas', 11)
$form.Controls.Add($tb)

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 200
$timer.Add_Tick({ try { [IO.File]::WriteAllText($TextFile, $tb.Text) } catch {} })
$form.Add_Shown({
    $tb.Focus()
    [IO.File]::WriteAllText($HwndFile, ("{0}`n{1}" -f $form.Handle.ToInt64(), $PID))
    $timer.Start()
})
[System.Windows.Forms.Application]::Run($form)
