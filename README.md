# taskbar-tools-rs

Configuration-driven Windows 10/11 Explorer taskbar tools in one process:

- **TaskbarPin**: automatically move configured application items to the head,
  preserving the relative order of everything else.
- **CenterTaskbar**: center legacy taskbar buttons, or use native centered
  alignment on Windows 11.
- **HiddenTaskbar**: hide/show the taskbar on the foreground window's monitor.

Run the single `taskbar-tools-rs.exe`. Its tray menu matches `desktop-tools-rs`:
Start with Windows, Reload config, Edit config, Unpin all, Suspend/Resume
hotkeys, and Quit. Unpin all pauses automatic TaskbarPin ordering until the next
config reload or process restart. Suspend hotkeys disables only the
HiddenTaskbar toggle; the tray stays available. Quit stops the modules and
restores their temporary visual changes.

## Configuration

All modules share `taskbar-tools.toml` next to the executable. It is created on
first run. On upgrade, values from the old per-module TOMLs are merged into the
new file when it does not exist yet; the old files are ignored and can be
deleted.

Every section is optional and falls back to the shown defaults:

```toml
[taskbar-pin]
enabled = true
poll_ms = 750
pin = []

[center-taskbar]
enabled = true
poll_ms = 100

[hidden-taskbar]
enabled = true
hotkey = "ctrl+alt+f9"
resize_snapped = true
```

### TaskbarPin

Pin matching accepts a full UIA identity, an AutomationId (including the last
component of dotted app IDs), a display name, an executable filename, or an
executable stem without `.exe`; case is ignored. Missing pinned items are
skipped. For example, both `wezterm` and `org.wezfurlong.wezterm` match the
WezTerm taskbar button. Physical dragging still depends on the current Explorer
build allowing that reordering.

### Reload behavior

The shared config reloads automatically. Changes to enable switches, poll
intervals, pin entries, hotkey and snapped-window handling apply while running.
A malformed edit is logged and the last valid values are kept.

## Diagnostics

```powershell
# Read-only TOML-friendly taskbar identities.
.\taskbar-tools-rs.exe --dump

# Read-only taskbar and centering discovery.
.\taskbar-tools-rs.exe --inspect

# Run one manual TaskbarPin pass, even when automatic pinning is disabled.
.\taskbar-tools-rs.exe --once
```

Diagnostics work in an existing terminal or captured pipe; a console created by
the daemon itself is hidden. One instance is allowed.

## Build

`cargo build --release`, `cargo test`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `just dist` (uses `just` and
PowerShell). Release uses LTO, one codegen unit and stripped symbols.
`just dist` creates one `dist/taskbar-tools-rs.zip` containing the executable,
README and the default `taskbar-tools.toml`.

Styling taskbar IME/timestamp indicators is a separate, low-priority design
task, not part of this implementation.
