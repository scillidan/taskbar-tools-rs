# center-taskbar

Centers taskbar buttons using the mechanism supported by the current shell:

- If `MSTaskListWClass` exists, refreshes legacy taskbar list positions with UIA
  bounds and `SetWindowPos`. Normal **Exit** restores Explorer's alignment.
- Otherwise, enables Explorer's native centered alignment (`TaskbarAl = 1`) and
  broadcasts `WM_SETTINGCHANGE`. It never moves Windows 11 XAML or restarts
  Explorer. Normal **Exit** restores the previous value only if the app changed it.

Run `center-taskbar.exe --inspect` for read-only discovery output. The tray menu
provides **Edit config**, **Start with Windows**, and **Exit**. The adjacent
`center-taskbar.toml` is created on first normal launch; `poll_ms` defaults to 100.

Physical verification still needs to cover primary/secondary bars, all four bar
orientations, mixed DPI, overflow boundaries, native alignment, and normal-exit
restoration on actual Windows 10 and Windows 11 systems.
