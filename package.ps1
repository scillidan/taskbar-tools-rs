$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$target = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$dist = Join-Path $root 'dist'
$app = 'taskbar-tools-rs'
$stage = Join-Path $dist $app
Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item $stage -ItemType Directory -Force | Out-Null
Copy-Item (Join-Path $target "release\$app.exe") $stage
Copy-Item (Join-Path $root 'README.md') $stage
foreach ($config in @('taskbar-pin', 'tray-pin', 'center-taskbar', 'start-button', 'hidden-taskbar')) {
    $source = if ($config -eq 'center-taskbar') {
        Join-Path $root "apps\$app\src\center_taskbar\default.toml"
    } else {
        Join-Path $root "apps\$app\$config.toml"
    }
    Copy-Item $source (Join-Path $stage "$config.toml")
}
Copy-Item (Join-Path $root 'assets') (Join-Path $stage 'assets') -Recurse
$zip = Join-Path $dist "$app.zip"
Compress-Archive (Join-Path $stage '*') $zip -Force
Write-Output $zip
