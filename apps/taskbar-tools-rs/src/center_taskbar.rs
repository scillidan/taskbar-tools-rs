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

#[cfg(windows)]
const APP_NAME: &str = "center-taskbar";
#[cfg(any(windows, test))]
const DEFAULT_CONFIG: &str = include_str!("center_taskbar/default.toml");

#[cfg(any(windows, test))]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "taskbar_kit::config::default_enabled")]
    enabled: bool,
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
pub fn run() -> Result<()> {
    use taskbar_kit::config;

    let config_path = config::exe_dir_file("center-taskbar.toml")?;
    let mut poll_ms = 100;
    let mut legacy = false;
    let mut legacy_lists = Vec::new();
    let mut native = None;
    platform::with_com(|| {
        while !crate::shutting_down() {
            let result = (|| -> Result<()> {
                let text = config::load_or_create(&config_path, DEFAULT_CONFIG)?;
                let config: Config =
                    toml::from_str(&text).context("invalid center-taskbar.toml")?;
                config.validate()?;
                poll_ms = config.poll_ms;
                if !config.enabled {
                    platform::restore_legacy(&legacy_lists);
                    legacy_lists.clear();
                    legacy = false;
                    native = None;
                    return Ok(());
                }
                if !legacy && native.is_none() {
                    if platform::discover()?.mode() == platform::Mode::Legacy {
                        legacy = true;
                    } else {
                        native = Some(platform::NativeAlignment::enable()?);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(config.enabled);
        assert_eq!(config.poll_ms, 100);
        config.validate().unwrap();
    }

    #[test]
    fn zero_poll_interval_is_rejected() {
        let config: Config = toml::from_str("poll_ms = 0").unwrap();
        assert!(config.validate().is_err());
    }
}
