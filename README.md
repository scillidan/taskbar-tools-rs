# taskbar-tools-rs

Configuration-driven Windows 10/11 Explorer taskbar tools in one process:

- **TaskbarPin / TrayPin**: automatically move configured application and tray
  items to the head, preserving the relative order of everything else.
- **CenterTaskbar**: center legacy taskbar buttons, or use native centered
  alignment on Windows 11.
- **StartButton**: theme-aware Start-button overlay with UIA click forwarding.
- **HiddenTaskbar**: hide/show the taskbar on the foreground window's monitor.

Run the single `taskbar-tools-rs.exe`. Its one tray icon provides editors for all
five configs, manual TaskbarPin/TrayPin tests and Quit. Quit stops the modules
and restores their temporary visual changes.

## Configuration

Each module has an independent TOML file next to the executable. Missing files
are created from defaults. Set `enabled = false` in a module's configuration to
disable it without disabling the other modules.

- `taskbar-pin.toml`: `enabled`, `poll_ms`, `pin`.
- `tray-pin.toml`: `enabled`, `poll_ms`, `pin`. Only visible tray icons are
  handled; use Explorer's own overflow UI for hidden icons.
- `center-taskbar.toml`: `enabled`, `poll_ms`.
- `start-button.toml`: `enabled`, polling, icon size and optional `.ico`/`.png`
  icons. Relative paths resolve beside the executable; bundled defaults use
  `assets/black_Lake.png` and `assets/white_Lake.png`.
- `hidden-taskbar.toml`: `general.enabled`, `general.hotkey`,
  `general.resize_snapped`.

Pin matching accepts a stable AutomationId, display name, executable filename,
or executable stem without `.exe`; case is ignored. Missing pinned items are
skipped. Example:

```toml
# taskbar-pin.toml
enabled = true
poll_ms = 750
pin = ["brave", "doublecmd", "sublime_text"]
```

The pin files reload automatically. CenterTaskbar reloads its enable switch and
poll interval. StartButton reloads its enable switch; other visual changes take
effect on process restart. HiddenTaskbar reloads its enable switch, hotkey and
snapped-window setting. HiddenTaskbar is the only module with a hotkey; its
default is `Ctrl+Alt+F9`.

The hidden tray overflow is temporarily opened and restored for an available
move. Physical dragging still depends on the current Explorer build allowing
that reordering. CenterTaskbar capability-detects legacy versus native layout;
it never restarts Explorer. When disabled, CenterTaskbar restores a native
alignment setting that it changed, StartButton removes its overlays, and
HiddenTaskbar restores its temporary window/taskbar changes.

## Diagnostics

```powershell
# Read-only TOML-friendly taskbar/tray identities.
.\taskbar-tools-rs.exe --dump

# Read-only taskbar, tray, centering and Start-button discovery.
.\taskbar-tools-rs.exe --inspect

# Run one manual pin pass for both modules, even when automatic pinning is disabled.
.\taskbar-tools-rs.exe --once
```

Diagnostics work in an existing terminal or captured pipe; a console created by
the daemon itself is hidden. One instance is allowed.

## Build

`cargo build --release`, `cargo test`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `just dist` (uses `just` and
PowerShell). Release uses LTO, one codegen unit and stripped symbols.
`just dist` creates one `dist/taskbar-tools-rs.zip` containing the executable,
README and five default configuration files.

Styling taskbar IME/timestamp indicators is a separate, low-priority design
task, not part of this implementation.
