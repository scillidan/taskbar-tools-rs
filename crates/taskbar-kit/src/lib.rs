//! Shared scaffolding for the taskbar tools.

pub mod config;
pub mod icon;
pub mod keys;
pub mod order;

#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
pub mod sorting;
#[cfg(windows)]
pub mod tray;
#[cfg(windows)]
pub mod win;

#[cfg(target_os = "linux")]
pub fn unimplemented(feature: &str) -> ! {
    eprintln!("{feature}: Windows Explorer taskbar features are unavailable on Linux");
    std::process::exit(2)
}
