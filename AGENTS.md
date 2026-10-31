# taskbar-tools-rs

Windows 10/11 Explorer taskbar utilities. One app per tool plus `taskbar-kit`.

## Commands

- `cargo build --release`, `cargo test`
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
- Config and runtime state live next to each exe as TOML; defaults use `include_str!`.
- Run read-only diagnostics with `--inspect` before any physical reorder test.
- Never restart Explorer or terminate the user's apps as a routine build step.

## Conventions

- anyhow errors with context; background loops log and continue after individual failures.
- Inline `#[cfg(test)]` tests; release: LTO, one codegen unit, stripped.
- Daemons hide only a self-owned console so release `--inspect`/`--dump` still
  work in terminals and captured pipes.
- Keep UIA/layout/order logic separate from physical mouse/Win32 operations.
- Windows 10 toolbar HWND techniques and Windows 11 XAML/UIA techniques must be
  capability-detected, never selected only by a hard-coded build-number table.
- No Explorer injection, registry icon-order editing, or undocumented shell DLL
  hooks. Unsupported operations must fail visibly rather than corrupt shell state.

## Verification

Every mutating app must restore its temporary visual changes on normal exit.
Pinning gets a read-only `--inspect` and config-ready `--dump`. Update
`CHECKLIST.md` with actual OS/build and distinguish automated from physical checks.
The low-priority IME/time styling remains design-only unless explicitly requested.
