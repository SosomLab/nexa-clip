$ErrorActionPreference = 'Stop'

# Portable - unzip into the package folder and create a shim only (no install footprint).
# Chocolatey picks the archive that matches the OS architecture.
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

Install-ChocolateyZipPackage `
  -PackageName 'nexa-clip-portable' `
  -Url         'https://github.com/SosomLab/nexa-clip/releases/download/v@VERSION@/nexa-clip-@VERSION@-windows-x64-portable.zip' `
  -Checksum    '@SHA_WIN_X64_PORTABLE@' -ChecksumType 'sha256' `
  -Url64bit    'https://github.com/SosomLab/nexa-clip/releases/download/v@VERSION@/nexa-clip-@VERSION@-windows-x64-portable.zip' `
  -Checksum64  '@SHA_WIN_X64_PORTABLE@' -ChecksumType64 'sha256' `
  -UnzipLocation $toolsDir

# nclip-imgdec (isolated image decode worker) is a helper the main executable launches from its own folder - no shim.
Get-ChildItem $toolsDir -Recurse -Filter 'nclip-imgdec.exe' | ForEach-Object {
  New-Item -ItemType File -Path "$($_.FullName).ignore" -Force | Out-Null
}
