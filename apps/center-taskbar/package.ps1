$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$dist = Join-Path $PSScriptRoot 'dist'
$stage = Join-Path $dist 'center-taskbar'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item $stage -ItemType Directory -Force | Out-Null
Copy-Item (Join-Path $target 'release\center-taskbar.exe') $stage
Copy-Item (Join-Path $PSScriptRoot 'README.md') $stage
Compress-Archive (Join-Path $stage '*') (Join-Path $dist 'center-taskbar.zip') -Force
Write-Output (Join-Path $dist 'center-taskbar.zip')
