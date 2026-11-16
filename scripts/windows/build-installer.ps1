# Builds the Windows installer for an already built visr.exe with Inno Setup 6.3 or later.
# The structure follows the Sonora script (GPL-3.0-or-later): scripts/windows/build-installer.ps1
# in https://github.com/atleugim/sonora.
# Run `scripts/licenses.sh target` first (it needs bash) and build with `cargo build --release`.
# Needs PowerShell 7.3 or later.
param(
  [Parameter(Mandatory)]
  [string]$Version,
  [ValidateSet("x64", "arm64")]
  [string]$Arch = "x64"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$root = Resolve-Path (Join-Path $PSScriptRoot "../..")
$source = Resolve-Path (Join-Path $root "target/release/visr.exe")
$outputPath = Join-Path $root "dist"
New-Item -ItemType Directory -Force $outputPath | Out-Null

$iscc = @(
  (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6/ISCC.exe")
  (Join-Path $env:LOCALAPPDATA "Programs/Inno Setup 6/ISCC.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
  throw "Inno Setup 6 is not installed. Install it with 'choco install innosetup' and run again."
}

# The installer ships the same licenses folder as the macOS bundle.
if (-not (Test-Path (Join-Path $root "target/licenses/LICENSE"))) {
  throw "Run scripts/licenses.sh target first."
}

$plainVersion = $Version.TrimStart("v")
$env:VISR_VERSION = $plainVersion
$env:VISR_EXE = $source.Path
$env:VISR_DIST = (Resolve-Path $outputPath).Path
# Inno Setup names the x64 architecture x64compatible.
$env:VISR_ARCH = if ($Arch -eq "x64") { "x64compatible" } else { "arm64" }
$env:VISR_SETUP = "visr-v$plainVersion-windows-$Arch"

& $iscc (Join-Path $PSScriptRoot "visr.iss")
