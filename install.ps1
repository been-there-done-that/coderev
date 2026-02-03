$ErrorActionPreference = "Stop"

$prefix = $env:Coderev_PREFIX
if ([string]::IsNullOrWhiteSpace($prefix)) {
  $prefix = "$env:ProgramFiles\Coderev"
}
$binDir = Join-Path $prefix "bin"
$binName = "coderev.exe"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Write-Error "cargo is required (install Rust)"
  exit 1
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null

cargo build --release
Copy-Item -Force "target\release\$binName" "$binDir\$binName"

Write-Host "Installed $binName to $binDir\$binName"
