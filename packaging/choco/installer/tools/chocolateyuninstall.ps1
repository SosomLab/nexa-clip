$ErrorActionPreference = 'Stop'

# Use the uninstall entry written by the installer (HKCU - it is a per-user install).
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\NexaClip'
if (Test-Path $key) {
  $uninst = (Get-ItemProperty $key).UninstallString -replace '"', ''
  if ($uninst -and (Test-Path $uninst)) {
    Uninstall-ChocolateyPackage -PackageName 'nexa-clip' -FileType 'exe' `
      -SilentArgs '/S' -File $uninst -ValidExitCodes @(0)
  }
} else {
  Write-Host 'Nexa Clip uninstall entry not found - assuming it is already removed.'
}
