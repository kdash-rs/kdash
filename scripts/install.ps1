# install.ps1 — install kdash from GitHub Releases on Windows.
#
# Usage:
#   irm https://raw.githubusercontent.com/kdash-rs/kdash/main/scripts/install.ps1 | iex
#   irm https://github.com/kdash-rs/kdash/releases/latest/download/install.ps1 | iex
#
# Parameters (named or environment):
#   -Version <vX.Y.Z>     Install a specific tag instead of the latest release.
#   -InstallDir <path>    Install into <path> instead of %LOCALAPPDATA%\Programs\kdash.
#   -Quiet                Suppress progress chatter; errors still print.
#   -AddToPath            Append InstallDir to the user PATH (idempotent).
#
# Environment variables (used as defaults when the parameter is unset):
#   KDASH_VERSION       Same as -Version.
#   KDASH_INSTALL_DIR   Same as -InstallDir.
#   KDASH_QUIET=1       Same as -Quiet.
#
# Exit codes mirror install.sh:
#   0   success
#   1   generic failure (download error, network, unknown)
#   2   checksum verification failed
#   64  unsupported platform or invalid usage
#
# kdash ships x86_64-pc-windows-msvc and aarch64-pc-windows-msvc as .tar.gz
# archives (extracted with the built-in tar.exe). No admin elevation is
# required — %LOCALAPPDATA% is per-user.

[CmdletBinding()]
param(
  [string]$Version = $env:KDASH_VERSION,
  [string]$InstallDir = $env:KDASH_INSTALL_DIR,
  [switch]$Quiet,
  [switch]$AddToPath
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'kdash-rs/kdash'
$BinName = 'kdash.exe'

# Test-only overrides; in production these resolve to GitHub's real endpoints.
$BaseUrl = if ($env:KDASH_BASE_URL) { $env:KDASH_BASE_URL } else { "https://github.com/$Repo/releases/download" }
$LatestUrl = if ($env:KDASH_LATEST_URL) { $env:KDASH_LATEST_URL } else { "https://api.github.com/repos/$Repo/releases/latest" }

if ([string]::IsNullOrEmpty($InstallDir)) {
  $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\kdash'
}

if (-not $Quiet -and $env:KDASH_QUIET -eq '1') {
  $Quiet = $true
}

function Write-Log {
  param([string]$Message)
  if (-not $Quiet) { Write-Host $Message }
}

function Exit-WithCode {
  param([int]$Code, [string]$Message)
  # Write straight to stderr and exit so the documented exit code is honored
  # regardless of $ErrorActionPreference (Write-Error would throw under 'Stop'
  # before `exit` runs). `exit` still triggers enclosing finally blocks.
  [Console]::Error.WriteLine("error: $Message")
  exit $Code
}

# ----- target detection -------------------------------------------------------

$arch = $env:PROCESSOR_ARCHITECTURE
if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }

# Map the host architecture to the kdash release-asset suffix.
$suffix = switch -Regex ($arch) {
  '^(AMD64|x86_64)$' { 'windows' }
  '^(ARM64|aarch64)$' { 'windows-aarch64' }
  default {
    Exit-WithCode 64 "unsupported Windows architecture: $arch (supported: AMD64, ARM64)"
  }
}

# tar.exe (bsdtar) ships with Windows 10 1803+ and is required to unpack the
# .tar.gz assets. Fail early with a clear message if it's missing.
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
  Exit-WithCode 1 'tar.exe not found on PATH (requires Windows 10 1803+). Install kdash with `cargo install kdash`, `scoop install kdash`, or `choco install kdash` instead.'
}

# ----- resolve version --------------------------------------------------------

if ([string]::IsNullOrEmpty($Version)) {
  Write-Log 'resolving latest release tag...'
  try {
    $release = Invoke-RestMethod -Uri $LatestUrl -Headers @{ 'User-Agent' = 'kdash-install.ps1' }
  } catch {
    Exit-WithCode 1 "could not fetch latest release metadata: $_"
  }
  $Version = $release.tag_name
  if ([string]::IsNullOrEmpty($Version)) {
    Exit-WithCode 1 'GH latest-release response has no tag_name'
  }
}

# Normalise: tags are 'vX.Y.Z'; accept a bare 'X.Y.Z' too.
if ($Version -notmatch '^v') { $Version = "v$Version" }

$AssetName = "kdash-$suffix.tar.gz"
$AssetUrl = "$BaseUrl/$Version/$AssetName"
$SumsUrl = "$BaseUrl/$Version/kdash-$suffix.sha256"

Write-Log "target:  $suffix"
Write-Log "version: $Version"
Write-Log "asset:   $AssetName"

# ----- idempotence ------------------------------------------------------------

$destBin = Join-Path $InstallDir $BinName
if (Test-Path $destBin) {
  try {
    $current = (& $destBin --version 2>$null | Select-Object -First 1)
    if ($current) {
      $currentVer = ($current -split '\s+')[-1]
      if ("v$currentVer" -eq $Version) {
        Write-Log "kdash $Version already installed at $destBin - nothing to do"
        exit 0
      }
    }
  } catch {
    # Fall through to a fresh install on any error.
  }
}

# ----- download + verify ------------------------------------------------------

$tmpDir = Join-Path $env:TEMP "kdash-install-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $tmpDir | Out-Null
try {
  $tarPath = Join-Path $tmpDir $AssetName
  $sumsPath = Join-Path $tmpDir 'kdash.sha256'

  Write-Log "downloading $AssetUrl..."
  try {
    Invoke-WebRequest -Uri $AssetUrl -OutFile $tarPath -UseBasicParsing
  } catch {
    Exit-WithCode 1 "could not download ${AssetName}: $_"
  }

  Write-Log 'downloading checksum...'
  try {
    Invoke-WebRequest -Uri $SumsUrl -OutFile $sumsPath -UseBasicParsing
  } catch {
    Exit-WithCode 1 "could not download checksum: $_"
  }

  # The kdash Windows sidecar is a bare 64-hex digest (certutil output filtered
  # to the hash line). Be tolerant of a trailing "  filename" too, by taking the
  # first 64-hex token found in the file.
  $sumsText = Get-Content -Raw $sumsPath
  $m = [regex]::Match($sumsText, '[a-fA-F0-9]{64}')
  if (-not $m.Success) {
    Exit-WithCode 2 "checksum file at $SumsUrl is malformed"
  }
  $expected = $m.Value.ToLower()
  $actual = (Get-FileHash -Algorithm SHA256 -Path $tarPath).Hash.ToLower()
  if ($actual -ne $expected) {
    Exit-WithCode 2 "SHA-256 mismatch: expected $expected, got $actual"
  }
  Write-Log 'checksum OK'

  # ----- extract --------------------------------------------------------------

  $extractDir = Join-Path $tmpDir 'extract'
  New-Item -ItemType Directory -Path $extractDir | Out-Null
  # kdash tarballs contain the bare kdash.exe at the archive root.
  & tar -xzf $tarPath -C $extractDir
  if ($LASTEXITCODE -ne 0) {
    Exit-WithCode 1 "tar failed to extract $AssetName"
  }

  $sourceBin = Get-ChildItem -Recurse -Path $extractDir -Filter $BinName | Select-Object -First 1
  if (-not $sourceBin) {
    Exit-WithCode 1 "extracted archive did not contain $BinName"
  }

  if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
  }
  Copy-Item -Path $sourceBin.FullName -Destination $destBin -Force
  Write-Log "installed: $destBin"

  # ----- PATH update (opt-in) -------------------------------------------------

  if ($AddToPath) {
    $userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    $entries = if ($userPath) { $userPath -split ';' } else { @() }
    if ($entries -notcontains $InstallDir) {
      $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
      [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')
      Write-Log "added $InstallDir to user PATH (open a new terminal to pick it up)"
    } else {
      Write-Log "$InstallDir already on user PATH"
    }
  } else {
    Write-Log "tip: add `"$InstallDir`" to your user PATH, or rerun with -AddToPath"
  }
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmpDir
}

Write-Log 'done. run `kdash --help` to get started.'
