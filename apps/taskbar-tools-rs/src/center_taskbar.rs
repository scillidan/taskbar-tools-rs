#[cfg(any(windows, test))]
mod geometry;
#[cfg(windows)]
mod platform;

#[cfg(windows)]
use anyhow::Result;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
const APP_NAME: &str = "center-taskbar";

#[cfg(windows)]
pub fn run() -> Result<()> {
    taskbar_kit::win::log(APP_NAME, "starting");
    let mut poll_ms = 100;
    let mut legacy = false;
    let mut legacy_lists = Vec::new();
    let mut native = None;
    let mut native_check = Instant::now();
    platform::with_com(|| {
        taskbar_kit::win::log(APP_NAME, "COM initialized");
        while !crate::shutting_down() {
            let result = (|| -> Result<()> {
                let config = crate::config::load()?.center_taskbar;
                config.validate()?;
                poll_ms = config.poll_ms;
                if !config.enabled {
                    platform::restore_legacy(&legacy_lists);
                    legacy_lists.clear();
                    legacy = false;
                    native = None;
                    return Ok(());
                }
                if !legacy {
                    if native.is_none() {
                        if platform::discover()?.mode() == platform::Mode::Legacy {
                            legacy = true;
                            taskbar_kit::win::log(APP_NAME, "legacy taskbar mode");
                        } else {
                            native = Some(platform::NativeAlignment::enable()?);
                            native_check = Instant::now();
                            taskbar_kit::win::log(APP_NAME, "native taskbar alignment mode");
                        }
                    } else if native_check.elapsed() >= Duration::from_secs(1) {
                        native_check = Instant::now();
                        if platform::discover()?.mode() == platform::Mode::Legacy {
                            if let Some(mut alignment) = native.take() {
                                alignment.restore()?;
                            }
                            legacy = true;
                            taskbar_kit::win::log(
                                APP_NAME,
                                "legacy task list appeared; switched from native alignment",
                            );
                        }
                    }
                }
                if legacy {
                    legacy_lists = platform::legacy_step()?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                taskbar_kit::win::log(APP_NAME, &format!("{error:#}"));
            }
            crate::wait_ms(poll_ms);
        }
        platform::restore_legacy(&legacy_lists);
        Ok(())
    })
}

#[cfg(windows)]
pub fn inspect() -> Result<()> {
    let discovery = platform::with_com(platform::discover)?;
    platform::print_discovery(&discovery);
    Ok(())
}
