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
use taskbar_kit::drag::{drag_move, first_move, taskbar_groups};
#[cfg(windows)]
use taskbar_kit::pin::pin_order;

#[cfg(windows)]
const APP: &str = "taskbar-pin";

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
    750
}

#[cfg(windows)]
#[derive(Serialize)]
struct Dump {
    order: Vec<String>,
}

#[cfg(windows)]
pub fn run() -> Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    let config_path = taskbar_kit::config::exe_dir_file("taskbar-pin.toml")?;
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
        &taskbar_kit::config::exe_dir_file("taskbar-pin.toml")?,
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
        order: taskbar_groups(automation)?
            .into_iter()
            .flat_map(|group| group.ids())
            .collect(),
    };
    print!("{}", toml::to_string_pretty(&dump)?);
    Ok(())
}

#[cfg(windows)]
pub fn inspect(automation: &taskbar_kit::shell::Automation) -> Result<()> {
    for bar in taskbar_kit::shell::taskbars()? {
        println!(
            "taskbar {:?} secondary={} rect={}",
            bar.hwnd, bar.secondary, bar.rect
        );
        for item in automation.descendants_of(bar.hwnd)? {
            println!(
                "  name={:?} id={:?} class={:?} type={} offscreen={} rect={}",
                item.name,
                item.automation_id,
                item.class_name,
                item.control_type,
                item.offscreen,
                item.rect
            );
        }
    }
    Ok(())
}

/// Check every taskbar and perform at most one physical move per group.
#[cfg(windows)]
fn tick(automation: &taskbar_kit::shell::Automation, cfg: &FileConfig) -> Result<bool> {
    let mut moved = false;
    for group in taskbar_groups(automation)? {
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
    let text = taskbar_kit::config::load_or_create(path, include_str!("../taskbar-pin.toml"))?;
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
    fn default_config_is_valid() {
        let cfg: FileConfig = toml::from_str(include_str!("../taskbar-pin.toml")).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.poll_ms, 750);
        assert!(cfg.pin.is_empty());
    }
}
