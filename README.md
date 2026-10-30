# taskbar-tools-rs

Small, independent Rust utilities for the Windows 10/11 Explorer taskbar.
Arch Linux has no matching Explorer surface, so these apps report that the
feature is unavailable there instead of pretending to support another shell.

| App | Purpose | Windows 10 | Windows 11 |
| --- | --- | --- | --- |
| `hidden-taskbar` | Toggle the taskbar on the foreground monitor | Win32 | Win32 |
| `center-taskbar` | Center application buttons | UIA/Win32 | native taskbar alignment |
| `tray-sort` | Sort visible and overflow notification icons separately | UIA | UIA |
| `taskbar-sort` | Sort application buttons | UIA | UIA |
| `start-button` | Theme-aware custom Start icon overlay | UIA + overlay | UIA + overlay |

## Build and run

```sh
cargo build --release
cargo test --workspace
just dist
```

`just dist` creates one zip per app under `dist/`. Each app has its own tray
icon, TOML config, single-instance guard, autostart toggle and normal **Exit**.
Config and custom-order state live beside the exe; defaults are created on first
normal launch. No app restarts or injects into Explorer, terminates user apps,
or edits undocumented binary registry state.

- **HiddenTaskbar:** default hotkey is `Ctrl+Alt+F9`; the tray menu also has
  **Toggle foreground** and **Restore all**. Normal exit restores hidden bars and
  snapped windows it moved.
- **CenterTaskbar:** uses `MSTaskListWClass` only when that legacy surface is
  present; otherwise it uses Explorer's native centered alignment. Restart after
  changing `poll_ms`.
- **TraySort / TaskbarSort:** `mode` is `alphabetical`,
  `alphabetical-desc`, or `custom`. `lock_head`/`lock_tail` accept case-insensitive
  UIA automation IDs or display names from `--dump`. System tray icons stay last.
  Config changes are reloaded; custom order is saved with a settling delay.
  Disable mouse movement with `apply = false` or the **Apply sorting** menu item.
- **StartButton:** `light_icon` and `dark_icon` are optional `.ico` paths, relative
  to the exe or absolute. Empty values use the built-in four-pane logo. Restart
  after changing icon files or `icon_size`.

## Read-only diagnostics

Run these from a terminal before enabling a sort:

```sh
taskbar-sort.exe --inspect
taskbar-sort.exe --dump
tray-sort.exe --inspect
tray-sort.exe --dump
start-button.exe --inspect
center-taskbar.exe --inspect
```

`--inspect` prints UIA identities and physical-pixel rectangles without moving
windows or icons. `--dump` emits TOML identities suitable for the adjacent custom
state file and for choosing lock entries. Overflow contents are enumerated only
when the overflow window is already open; diagnostics never open it themselves.

`--once` on either sorting app is explicitly mutating: it performs physical mouse
drags. Hidden overflow sorting likewise happens only while that window is open.
This is the external-UIA boundary; reliable sorting while it is closed would
require version-fragile Explorer integration that this project deliberately avoids.

The IME and clock styling ideas remain design-only. Replacing current XAML shell
controls needs unsupported Explorer hooks rather than a stable external API.

See `CHECKLIST.md` for automated versus physical verification.
