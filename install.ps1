$ErrorActionPreference = "Stop"

$prefix = $env:Coderev_PREFIX
if ([string]::IsNullOrWhiteSpace($prefix)) {
  $prefix = "$env:ProgramFiles\Coderev"
}
$binDir = Join-Path $prefix "bin"
$binName = "coderev.exe"

# Get Latest Version from GitHub API
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/been-there-done-that/coderev/releases/latest"
$version = $release.tag_name

if (-not $version) {
    Write-Error "Could not find latest release version"
    exit 1
}

$assetName = "coderev-x86_64-pc-windows-msvc.exe"
$downloadUrl = "https://github.com/been-there-done-that/coderev/releases/download/$version/$assetName"

Write-Host "Downloading $binName $version..."
$tmpDir = [System.IO.Path]::GetTempPath()
$tmpFile = Join-Path $tmpDir $assetName

try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tmpFile -ErrorAction Stop
} catch {
    Write-Error "Failed to download $downloadUrl"
    exit 1
}

if (-not (Test-Path $binDir)) {
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
}

Copy-Item -Force $tmpFile (Join-Path $binDir $binName)

Write-Host "Successfully installed $binName to $binDir\$binName"
