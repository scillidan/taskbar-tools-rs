#[cfg(any(windows, test))]
mod geometry;
#[cfg(windows)]
mod platform;

#[cfg(windows)]
use anyhow::Context;
#[cfg(any(windows, test))]
use anyhow::Result;
#[cfg(any(windows, test))]
use serde::Deserialize;

const APP_NAME: &str = "center-taskbar";
#[cfg(any(windows, test))]
const DEFAULT_CONFIG: &str = include_str!("default.toml");

#[cfg(any(windows, test))]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "default_poll_ms")]
    poll_ms: u64,
}

#[cfg(any(windows, test))]
const fn default_poll_ms() -> u64 {
    100
}

#[cfg(any(windows, test))]
impl Config {
    fn validate(&self) -> Result<()> {
        if self.poll_ms < 10 {
            anyhow::bail!("poll_ms must be at least 10");
        }
        Ok(())
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        let text = format!("{error:#}");
        eprintln!("{APP_NAME}: {text}");
        taskbar_kit::win::message_box(APP_NAME, &text);
    }
}

#[cfg(target_os = "linux")]
fn main() {
    taskbar_kit::unimplemented(APP_NAME);
}

#[cfg(not(any(windows, target_os = "linux")))]
fn main() {
    eprintln!("{APP_NAME}: only Windows is implemented");
    std::process::exit(2);
}

#[cfg(windows)]
fn run() -> Result<()> {
    use std::sync::mpsc;
    use std::time::Duration;
    use taskbar_kit::tray::{Event, Tray};
    use taskbar_kit::win;
    use taskbar_kit::{config, icon};

    platform::initialize_process()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let inspect = match args.as_slice() {
        [] => false,
        [arg] if arg == "--inspect" => true,
        _ => anyhow::bail!("usage: center-taskbar [--inspect]"),
    };
    if inspect {
        attach_parent_console();
        let discovery = platform::with_com(platform::discover)?;
        platform::print_discovery(&discovery);
        return Ok(());
    }

    win::hide_owned_console();
    let _instance = win::ensure_single_instance(APP_NAME)?;
    let config_path = config::exe_dir_file("center-taskbar.toml")?;
    let text = config::load_or_create(&config_path, DEFAULT_CONFIG)?;
    let config: Config = toml::from_str(&text).context("invalid center-taskbar.toml")?;
    config.validate()?;

    let discovery = platform::with_com(platform::discover)?;
    let mode = discovery.mode();
    let mut tray = Tray::new(
        APP_NAME,
        icon::letter_rgba('C', [0x3b, 0x6e, 0xa5, 0xff], [0x23, 0x46, 0x6e, 0xff]),
    )?;
    let mut autostart = win::autostart_enabled(APP_NAME);
    tray.set_menu(&menu(autostart));

    let mut native = match mode {
        platform::Mode::Native => Some(platform::NativeAlignment::enable()?),
        platform::Mode::Legacy => None,
    };
    let (stop_tx, worker) = match mode {
        platform::Mode::Legacy => {
            let (tx, rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                platform::legacy_worker(Duration::from_millis(config.poll_ms), rx)
            });
            (Some(tx), Some(worker))
        }
        platform::Mode::Native => (None, None),
    };

    tray.run(None, |tray, event| {
        let Event::Menu(label) = event else {
            return;
        };
        match label.as_str() {
            "Edit config" => win::open_with_default(&config_path),
            "Start with Windows" => match win::set_autostart(APP_NAME, !autostart) {
                Ok(()) => {
                    autostart = !autostart;
                    tray.set_menu(&menu(autostart));
                }
                Err(error) => win::message_box(APP_NAME, &format!("{error:#}")),
            },
            "Exit" => tray.quit(),
            _ => {}
        }
    });

    if let Some(tx) = stop_tx {
        let _ = tx.send(());
    }
    if let Some(worker) = worker {
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("layout worker panicked"))??;
    }
    if let Some(native) = native.as_mut() {
        native.restore()?;
    }
    Ok(())
}

#[cfg(windows)]
fn menu(autostart: bool) -> Vec<taskbar_kit::tray::Entry> {
    use taskbar_kit::tray::Entry;
    vec![
        Entry::Item("Edit config"),
        Entry::Toggle("Start with Windows", autostart),
        Entry::Sep,
        Entry::Item("Exit"),
    ]
}

#[cfg(windows)]
fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(config.poll_ms, 100);
        config.validate().unwrap();
    }

    #[test]
    fn zero_poll_interval_is_rejected() {
        let config: Config = toml::from_str("poll_ms = 0").unwrap();
        assert!(config.validate().is_err());
    }
}
