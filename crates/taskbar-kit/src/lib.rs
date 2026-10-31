//! Shared scaffolding for the taskbar tools.

pub mod config;
pub mod keys;
pub mod pin;

#[cfg(windows)]
pub mod drag;
#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod message;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
pub mod win;

#[cfg(target_os = "linux")]
pub fn unimplemented(feature: &str) -> ! {
    eprintln!("{feature}: Windows Explorer taskbar features are unavailable on Linux");
    std::process::exit(2)
}
