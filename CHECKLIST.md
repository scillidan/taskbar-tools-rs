# Verification checklist

Last run: 2026-11-01, Windows 11 build 26200.8246, horizontal primary taskbar
(2560x30).

## Scope

- One `taskbar-tools-rs.exe` hosts TaskbarPin, CenterTaskbar and HiddenTaskbar.
- One grouped `taskbar-tools.toml` provides all per-module settings and enable
  switches.
- The tray menu matches `desktop-tools-rs`: Start with Windows, Reload config,
  Edit config, Unpin all, Suspend/Resume hotkeys, separator, Quit.
- StartButton is removed completely, including its module, configuration,
  resources and packaged assets.
- TrayPin is removed completely.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed on Windows.
- `cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`: passed.
- `cargo test --workspace`: 35 tests passed, including exact tray-menu labels,
  grouped-config parsing, removed-section compatibility, legacy-config
  migration, WezTerm dotted app-ID matching, centered task-list scoping and
  plain-message dispatch.
- `cargo build --release --workspace --target-dir <isolated>`: passed; one
  application executable.
- `package.ps1`: passed with the isolated target directory.
- `taskbar-tools-rs.exe --dump`: passed with exit code 0 and listed only the
  seven taskbar applications, not notification-area icons, while CenterTaskbar
  had shifted the task list.
- Packaging refuses to start while the packaged executable is running, and a
  successful package preserves the deployed TOMLs in the staging directory.
- `taskbar-tools-rs.exe --inspect`: passed with exit code 0; observed a legacy
  horizontal taskbar and WezTerm's dotted AutomationId.
- The repository contains no Chinese text in tracked source, configuration or
  documentation files.

## Physical verification required

### Tray and configuration

- [ ] Start with Windows toggles the HKCU Run value and survives a sign-out.
- [ ] Reload config re-enables automatic pinning after Unpin all.
- [ ] Edit config starts the configured `EDITOR`/`VISUAL` immediately, without
      an Open With dialog or stalling the tray.
- [ ] Unpin all stops automatic TaskbarPin ordering until Reload config or
      restart; it never edits `taskbar-tools.toml`.
- [ ] Suspend hotkeys disables the HiddenTaskbar hotkey; Resume hotkeys restores
      it immediately.
- [ ] Quit returns promptly, restores HiddenTaskbar and restores CenterTaskbar
      without changing pinned item order.
- [ ] Existing per-module TOMLs migrate into a missing grouped config exactly
      once; later edits use only `taskbar-tools.toml`.
- [ ] An old `[start-button]` section is ignored without an error.

### TaskbarPin

- [ ] `pin = ["wezterm"]` moves the observed `org.wezfurlong.wezterm` button to
      the head.
- [ ] Other `pin` items move to the head in configured order after startup.
- [ ] New and manually moved items return pinned items to the head without
      sorting the remaining items.
- [ ] `enabled = false` stops only the intended pin module.

### CenterTaskbar

- [ ] Vertical taskbar buttons center within the available bounds.
- [ ] `enabled = false` and Quit restore the original list position.
- [ ] Primary/secondary bars, four orientations and mixed DPI retain boundaries.

### HiddenTaskbar

- [ ] Configured hotkey toggles the taskbar with a WezTerm window foreground.
- [ ] Configured hotkey toggles only the foreground window's monitor.
- [ ] `enabled = false` and Quit restore taskbar/window changes.

No daemon was launched and no taskbar/tray/window changes were made during
automated checks.
