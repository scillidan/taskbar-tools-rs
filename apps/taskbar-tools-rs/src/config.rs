use anyhow::{Context, Result};
use serde::Deserialize;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
const FILE_NAME: &str = "taskbar-tools.toml";
const DEFAULT: &str = include_str!("../taskbar-tools.toml");
#[cfg(windows)]
static LOAD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "taskbar-pin")]
    pub taskbar_pin: TaskbarPin,
    #[serde(rename = "center-taskbar")]
    pub center_taskbar: CenterTaskbar,
    #[serde(rename = "hidden-taskbar")]
    pub hidden_taskbar: HiddenTaskbar,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct TaskbarPin {
    pub enabled: bool,
    pub poll_ms: u32,
    pub pin: Vec<String>,
}

impl Default for TaskbarPin {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_ms: 750,
            pin: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CenterTaskbar {
    pub enabled: bool,
    pub poll_ms: u64,
}

impl Default for CenterTaskbar {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_ms: 100,
        }
    }
}

impl CenterTaskbar {
    pub fn validate(&self) -> Result<()> {
        if self.poll_ms < 10 {
            anyhow::bail!("center-taskbar poll_ms must be at least 10");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct HiddenTaskbar {
    pub enabled: bool,
    pub hotkey: String,
    pub resize_snapped: bool,
}

impl Default for HiddenTaskbar {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: "ctrl+alt+f9".to_string(),
            resize_snapped: true,
        }
    }
}

#[cfg(windows)]
pub fn path() -> Result<PathBuf> {
    taskbar_kit::config::exe_dir_file(FILE_NAME)
}

/// Read the shared config, creating the default (or migrating old files) once.
#[cfg(windows)]
pub fn load() -> Result<Config> {
    let _guard = LOAD_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let path = path()?;
    let text = if path.exists() {
        std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?
    } else {
        let text = default_text(&path)?;
        std::fs::write(&path, &text)
            .with_context(|| format!("cannot create {}", path.display()))?;
        text
    };
    parse(&text).with_context(|| format!("cannot parse {}", path.display()))
}

fn parse(text: &str) -> Result<Config> {
    toml::from_str(text).context("invalid taskbar-tools.toml")
}

#[cfg(windows)]
fn default_text(path: &Path) -> Result<String> {
    let mut root: toml::Value = DEFAULT.parse().context("invalid built-in config")?;
    let Some(table) = root.as_table_mut() else {
        anyhow::bail!("built-in config root is not a table");
    };
    let dir = path.parent().context("config has no parent directory")?;
    for (name, section) in [
        ("taskbar-pin.toml", "taskbar-pin"),
        ("center-taskbar.toml", "center-taskbar"),
        ("hidden-taskbar.toml", "hidden-taskbar"),
    ] {
        let old = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&old) else {
            continue;
        };
        let Ok(value) = text.parse::<toml::Value>() else {
            eprintln!("config: ignoring invalid {}", old.display());
            continue;
        };
        let value = if section == "hidden-taskbar" {
            value.get("general").cloned().unwrap_or(value)
        } else {
            value
        };
        table.insert(section.to_string(), value);
    }
    toml::to_string_pretty(&root).context("cannot serialize migrated config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = parse(DEFAULT).unwrap();
        assert!(cfg.taskbar_pin.enabled);
        assert_eq!(cfg.taskbar_pin.poll_ms, 750);
        assert!(cfg.taskbar_pin.pin.is_empty());
        assert!(cfg.center_taskbar.enabled);
        assert_eq!(cfg.center_taskbar.poll_ms, 100);
        assert!(cfg.hidden_taskbar.enabled);
        assert_eq!(cfg.hidden_taskbar.hotkey, "ctrl+alt+f9");
        assert!(cfg.hidden_taskbar.resize_snapped);
    }

    #[test]
    fn ignores_removed_start_section() {
        parse("[start-button]\nenabled = false\nicon_size = 32\n").unwrap();
    }

    #[test]
    fn missing_sections_keep_defaults() {
        let cfg = parse("[center-taskbar]\npoll_ms = 200\n").unwrap();
        assert_eq!(cfg.center_taskbar.poll_ms, 200);
        assert_eq!(cfg.taskbar_pin.poll_ms, 750);
        assert_eq!(cfg.hidden_taskbar.hotkey, "ctrl+alt+f9");
    }

    #[cfg(windows)]
    #[test]
    fn migrates_old_per_module_configs() {
        let dir = std::env::temp_dir().join(format!("taskbar-tools-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("taskbar-pin.toml"),
            "enabled = false\npoll_ms = 900\npin = [\"wezterm\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("hidden-taskbar.toml"),
            "[general]\nenabled = false\nhotkey = \"ctrl+alt+f10\"\nresize_snapped = false\n",
        )
        .unwrap();
        let text = default_text(&dir.join(FILE_NAME)).unwrap();
        let cfg = parse(&text).unwrap();
        assert!(!cfg.taskbar_pin.enabled);
        assert_eq!(cfg.taskbar_pin.poll_ms, 900);
        assert_eq!(cfg.taskbar_pin.pin, ["wezterm"]);
        assert!(!cfg.hidden_taskbar.enabled);
        assert_eq!(cfg.hidden_taskbar.hotkey, "ctrl+alt+f10");
        assert!(!cfg.hidden_taskbar.resize_snapped);
        assert_eq!(cfg.center_taskbar.poll_ms, 100);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn center_poll_interval_is_validated() {
        assert!(parse("[center-taskbar]\npoll_ms = 10\n")
            .unwrap()
            .center_taskbar
            .validate()
            .is_ok());
        assert!(parse("[center-taskbar]\npoll_ms = 9\n")
            .unwrap()
            .center_taskbar
            .validate()
            .is_err());
    }
}
