# emux installer for Windows
#
# Usage:
#   irm https://raw.githubusercontent.com/IISweetHeartII/emux/main/install.ps1 | iex
#
$ErrorActionPreference = "Stop"

$Repo = "IISweetHeartII/emux"
$InstallDir = "$env:LOCALAPPDATA\emux\bin"

# --- Detect architecture ---
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" { $Target = "aarch64-pc-windows-msvc" }
    default { Write-Error "Unsupported architecture: $Arch"; exit 1 }
}

# --- Get latest version ---
Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name

if (-not $Version) {
    Write-Error "Could not determine latest version."
    exit 1
}

Write-Host "Installing emux $Version for $Target..."

# --- Download and extract ---
$Url = "https://github.com/$Repo/releases/download/$Version/emux-$Version-$Target.zip"
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "emux-install"
$ZipPath = Join-Path $TmpDir "emux.zip"

New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
Invoke-WebRequest -Uri $Url -OutFile $ZipPath
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

# --- Install ---
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Move-Item -Force (Join-Path $TmpDir "emux.exe") (Join-Path $InstallDir "emux.exe")

# --- Add to PATH if not already ---
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host ""
    Write-Host "Added $InstallDir to your PATH."
    Write-Host "Restart your terminal for PATH changes to take effect."
}

# --- Cleanup ---
Remove-Item -Recurse -Force $TmpDir

Write-Host ""
Write-Host "emux $Version installed to $InstallDir\emux.exe"
Write-Host ""
Write-Host "Run 'emux' to get started."
