$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$target = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$dist = Join-Path $root 'dist'
$app = 'taskbar-tools-rs'
$stage = Join-Path $dist $app
$stageExe = Join-Path $stage "$app.exe"
$running = Get-Process -Name $app -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $stageExe }
if ($running) {
    throw "$stageExe is running; quit it before packaging."
}
$saved = Join-Path ([IO.Path]::GetTempPath()) "$app-config-backup"
Remove-Item $saved -Recurse -Force -ErrorAction SilentlyContinue
$configNames = @('taskbar-tools.toml', 'taskbar-pin.toml', 'center-taskbar.toml', 'hidden-taskbar.toml')
$configs = @(Get-ChildItem $stage -Filter '*.toml' -File -ErrorAction SilentlyContinue |
    Where-Object { $configNames -contains $_.Name })
if ($configs.Count -gt 0) {
    New-Item $saved -ItemType Directory -Force | Out-Null
    $configs | Copy-Item -Destination $saved
}
Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item $stage -ItemType Directory -Force | Out-Null
Copy-Item (Join-Path $target "release\$app.exe") $stage
Copy-Item (Join-Path $root 'README.md') $stage
Copy-Item (Join-Path $root "apps\$app\taskbar-tools.toml") $stage
$zip = Join-Path $dist "$app.zip"
Compress-Archive (Join-Path $stage '*') $zip -Force
if ($configs.Count -gt 0) {
    Remove-Item (Join-Path $stage '*.toml') -Force
    Copy-Item (Join-Path $saved '*') $stage
    Remove-Item $saved -Recurse -Force
}
Write-Output $zip
