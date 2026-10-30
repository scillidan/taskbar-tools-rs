$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$apps = @(
    @{ Name = 'hidden-taskbar'; Config = 'hidden-taskbar.toml' },
    @{ Name = 'center-taskbar'; Config = 'src\default.toml' },
    @{ Name = 'tray-sort'; Config = 'tray-sort.toml' },
    @{ Name = 'taskbar-sort'; Config = 'taskbar-sort.toml' },
    @{ Name = 'start-button'; Config = 'start-button.toml' }
)
foreach ($app in $apps) {
    $name = $app.Name
    $appDir = Join-Path $root "apps\$name"
    $stage = Join-Path $root "dist\$name"
    Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
    New-Item $stage -ItemType Directory -Force | Out-Null
    Copy-Item (Join-Path $target "release\$name.exe") $stage
    Copy-Item (Join-Path $appDir $app.Config) (Join-Path $stage "$name.toml")
    $readme = Join-Path $appDir 'README.md'
    if (-not (Test-Path $readme)) { $readme = Join-Path $root 'README.md' }
    Copy-Item $readme (Join-Path $stage 'README.md')
    # A fresh exe copy can be briefly locked by the realtime malware scanner.
    foreach ($attempt in 1..10) {
        try {
            Compress-Archive (Join-Path $stage '*') (Join-Path $root "dist\$name.zip") -Force
            break
        }
        catch {
            if ($attempt -eq 10) { throw }
            Start-Sleep -Milliseconds 300
        }
    }
    Write-Output "$name.zip"
}
