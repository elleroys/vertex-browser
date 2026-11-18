[CmdletBinding()]
param(
    [switch]$Launch,
    [string]$Url = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$nativeDir = Join-Path $repoRoot "native"
$targetExe = Join-Path $nativeDir "target\release\vertex-native.exe"
$distDir = Join-Path $repoRoot "dist"
$packageDir = Join-Path $distDir "Vortex-Windows-x64"
$zipPath = Join-Path $distDir "Vortex-Windows-x64.zip"

function Require-Command([string]$Name, [string]$Hint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found. $Hint"
    }
}

Require-Command "cargo" "Install Rust using rustup and reopen PowerShell."
Require-Command "rustc" "Install Rust using rustup and reopen PowerShell."

Write-Host "Vortex native build" -ForegroundColor Cyan
Write-Host ("Rust: " + (& rustc --version))
Write-Host ("Cargo: " + (& cargo --version))
Write-Host "Building Servo browser in release mode. The first build can take a long time..." -ForegroundColor Yellow

Push-Location $nativeDir
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path $targetExe -PathType Leaf)) {
    throw "Build reported success, but executable was not found at $targetExe"
}

New-Item -ItemType Directory -Force -Path $distDir | Out-Null
if (Test-Path $packageDir) { Remove-Item -Recurse -Force $packageDir }
New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
Copy-Item $targetExe (Join-Path $packageDir "Vortex.exe")
@'
Vortex Browser — Windows x64 development build

Run:
  Vortex.exe
  Vortex.exe https://example.com

The browser stores its profile under:
  %APPDATA%\VertexBrowser\profile.sqlite3

This is a development build. First startup may take time while Servo initializes.
If Windows reports a missing Visual C++ runtime, install the latest Microsoft Visual C++ 2015–2022 Redistributable (x64).

Security note:
OpenPhish Community is optional, disabled by default, and restricted to non-commercial use by its provider. Review the provider terms before enabling it.
'@ | Set-Content -Encoding UTF8 (Join-Path $packageDir "RUN-WINDOWS.txt")

if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host "Build complete: $zipPath" -ForegroundColor Green

if ($Launch) {
    $arguments = @()
    if (-not [string]::IsNullOrWhiteSpace($Url)) { $arguments += $Url }
    Start-Process -FilePath (Join-Path $packageDir "Vortex.exe") -ArgumentList $arguments
}
