#[cfg(windows)]
const APP: &str = "tray-pin";

#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(any(windows, test))]
use serde::Deserialize;
#[cfg(windows)]
use serde::Serialize;
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::time::{Duration, SystemTime};
#[cfg(windows)]
use taskbar_kit::drag::{drag_move, first_move, tray_groups};
#[cfg(windows)]
use taskbar_kit::pin::pin_order;

#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct FileConfig {
    #[serde(default = "taskbar_kit::config::default_enabled")]
    enabled: bool,
    #[serde(default = "default_poll")]
    poll_ms: u32,
    #[serde(default)]
    pin: Vec<String>,
}

#[cfg(any(windows, test))]
const fn default_poll() -> u32 {
    600
}

#[cfg(windows)]
#[derive(Serialize)]
struct Dump {
    visible: Vec<String>,
}

#[cfg(windows)]
pub fn run() -> Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    let config_path = taskbar_kit::config::exe_dir_file("tray-pin.toml")?;
    let mut config_modified = None;
    let mut cfg = load_config(&config_path, &mut config_modified)?;
    while !crate::shutting_down() {
        let result = (|| -> Result<()> {
            reload_if_changed(&config_path, &mut config_modified, &mut cfg)?;
            if cfg.enabled {
                tick(&automation, &cfg)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            taskbar_kit::win::log(APP, &format!("{error:#}"));
        }
        crate::wait_ms(u64::from(cfg.poll_ms.max(100)));
    }
    Ok(())
}

#[cfg(windows)]
pub fn once(automation: &taskbar_kit::shell::Automation) -> Result<()> {
    let mut modified = None;
    let cfg = load_config(
        &taskbar_kit::config::exe_dir_file("tray-pin.toml")?,
        &mut modified,
    )?;
    for _ in 0..100 {
        if !tick(automation, &cfg)? {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Ok(())
}

#[cfg(windows)]
pub fn dump(automation: &taskbar_kit::shell::Automation) -> Result<()> {
    let dump = Dump {
        visible: tray_groups(automation)?
            .into_iter()
            .flat_map(|group| group.ids())
            .collect(),
    };
    print!("{}", toml::to_string_pretty(&dump)?);
    Ok(())
}

#[cfg(windows)]
fn tick(automation: &taskbar_kit::shell::Automation, cfg: &FileConfig) -> Result<bool> {
    let mut moved = false;
    for group in tray_groups(automation)? {
        if group.items.is_empty() || cfg.pin.is_empty() {
            continue;
        }
        let target = pin_order(&group.pin_items(), &cfg.pin);
        if let Some((id, index)) = first_move(&group.ids(), &target) {
            drag_move(&group, &id, index)
                .with_context(|| format!("moving {id} in {}", group.key))?;
            moved = true;
        }
    }
    Ok(moved)
}

#[cfg(windows)]
fn load_config(path: &Path, modified: &mut Option<SystemTime>) -> Result<FileConfig> {
    let text = taskbar_kit::config::load_or_create(path, include_str!("../tray-pin.toml"))?;
    *modified = taskbar_kit::config::modified(path);
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

#[cfg(windows)]
fn reload_if_changed(
    path: &Path,
    modified: &mut Option<SystemTime>,
    cfg: &mut FileConfig,
) -> Result<()> {
    let now = taskbar_kit::config::modified(path);
    if now != *modified {
        *cfg = load_config(path, modified)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_pins_only_visible_icons() {
        let cfg: FileConfig = toml::from_str(include_str!("../tray-pin.toml")).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.poll_ms, 600);
        assert!(cfg.pin.is_empty());
    }
}
