# Verification checklist

Last run: 2026-10-29, Windows 11 build 26200.8246, vertical left taskbar.

## Scope

- One `taskbar-tools-rs.exe` hosts TaskbarPin, TrayPin, CenterTaskbar,
  StartButton and HiddenTaskbar.
- Five independent TOML files provide per-module `enabled` switches.
- One tray icon provides five config editors, manual Pin tests and Quit.
- TrayPin handles only visible notification-area icons; Explorer owns overflow.
- Orientation-dependent in-memory PNG padding is feasible but not implemented;
  source images are unchanged and no padding values have been selected yet.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed on Windows.
- `cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`: passed.
- `cargo test --workspace`: 33 tests passed, including real PNG-to-HICON conversion.
- `cargo build --release --workspace`: passed; one application executable.
- `package.ps1`: passed; one `dist/taskbar-tools-rs.zip` with one exe, README,
  five default TOML files and the assets directory.
- `taskbar-tools-rs.exe --dump`: passed with exit code 0; Keypirinha was absent
  from the visible tray and the old config's root `pin` was empty.
- `taskbar-tools-rs.exe --inspect`: passed with exit code 0; observed legacy
  taskbar bounds and the Start button.
- Start overlay creation, repaint, stale cleanup and Quit were audited to use the
  overlay HWND, never the taskbar HWND stored as the map key.
- No non-ASCII or Chinese text occurs in Rust source.

## Physical verification required

### Tray and configuration

- [ ] Each Edit command starts the configured `EDITOR`/`VISUAL` immediately,
      without an Open With dialog or blocking the tray.
- [ ] Test TaskbarPin/TrayPin works with automatic pinning disabled.
- [ ] Quit returns promptly, removes Start overlays, restores HiddenTaskbar and
      restores CenterTaskbar without changing pinned item order.

### TaskbarPin / TrayPin

- [ ] `pin` items move to the head in configured order after startup.
- [ ] New and manually moved items return pinned items to the head without
      sorting the remaining items.
- [ ] `pin = ["keypirinha"]` works after Keypirinha is visibly present in the
      system notification area; no overflow manipulation is expected.
- [ ] `enabled = false` stops only the intended pin module.

### CenterTaskbar

- [ ] Vertical taskbar buttons center within the available bounds.
- [ ] `enabled = false` and Quit restore the original list position.
- [ ] Primary/secondary bars, four orientations and mixed DPI retain boundaries.

### StartButton

- [ ] The overlay paints on its first and subsequent synchronization passes.
- [ ] Light/dark PNG and ICO icons render correctly; clicks open Start once.
- [ ] `enabled = false` and Quit destroy only overlays, never Explorer windows.
- [ ] Confirm horizontal and vertical padding before implementing in-memory
      orientation-dependent image margins.

### HiddenTaskbar

- [ ] Configured hotkey toggles only the foreground window's monitor.
- [ ] `general.enabled = false` and Quit restore taskbar/window changes.

No daemon was launched and no taskbar/tray/window changes were made during
automated checks.
