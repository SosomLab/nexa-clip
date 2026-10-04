$ErrorActionPreference = 'Stop'

# Per-user NSIS installer - silent install (/S) works without elevation.
$packageArgs = @{
  packageName    = 'nexa-clip'
  fileType       = 'exe'
  url            = 'https://github.com/SosomLab/nexa-clip/releases/download/v@VERSION@/nexa-clip-@VERSION@-windows-x64-setup.exe'
  checksum       = '@SHA_WIN_X64_SETUP@'
  checksumType   = 'sha256'
  url64bit       = 'https://github.com/SosomLab/nexa-clip/releases/download/v@VERSION@/nexa-clip-@VERSION@-windows-x64-setup.exe'
  checksum64     = '@SHA_WIN_X64_SETUP@'
  checksumType64 = 'sha256'
  silentArgs     = '/S'
  validExitCodes = @(0)
}
Install-ChocolateyPackage @packageArgs
