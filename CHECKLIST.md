# Verification checklist

Automated results and physical desktop verification are recorded separately.
Current development host: Windows 11 26H2, build 26300. Last run: 2026-10-26.

## Automated

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --offline -- -D warnings`
- [x] `cargo test --workspace --offline` — 30 tests passed
- [x] Linux `x86_64-unknown-linux-gnu` clippy, all targets, warnings denied
- [x] release build is newer than all source files
- [x] `just dist` — all five zip packages created under `dist/`

## Read-only shell probes

- [x] Release `--inspect` finds taskbars, Start, taskbar buttons and tray items
- [x] Release diagnostics return exit code 0 and produce captured terminal output
- [x] Release `--dump` emits taskbar and tray TOML identities
- [x] CenterTaskbar selects `Native` on the current Windows 11 host
- [x] StartButton UIA-first probe regression: no legacy-first `0xC0000005`
- [ ] Vertical and horizontal taskbars return usable physical-pixel rectangles
- [ ] Light/dark selection follows the effective taskbar theme on the real desktop

## Physical (user)

- [ ] HiddenTaskbar: hotkey affects only the foreground monitor; snapped windows restore
- [ ] CenterTaskbar: Windows 10 horizontal/vertical and multiple DPI monitors
- [ ] CenterTaskbar: Windows 11 native centering and normal-exit restoration
- [ ] TraySort: visible/hidden alphabetical, reverse, custom and locked groups
- [ ] TraySort: minimize-to-overflow delay and drag stability on a vertical taskbar
- [ ] TaskbarSort: all three modes and locked groups
- [ ] StartButton: four edges, light/dark, DPI changes and normal Start click
- [ ] Every app: normal Exit removes temporary overlays and restores shell state

No physical icon drag, taskbar hide, Start click, or native-alignment mutation was
performed during automated verification. Hidden overflow remains untested because
the read-only probe intentionally does not open it.
