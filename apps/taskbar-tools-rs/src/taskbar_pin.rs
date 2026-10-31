#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use serde::Serialize;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use taskbar_kit::drag::{drag_move, first_move, taskbar_groups};
#[cfg(windows)]
use taskbar_kit::pin::pin_order;

#[cfg(windows)]
const APP: &str = "taskbar-pin";

#[cfg(windows)]
#[derive(Serialize)]
struct Dump {
    order: Vec<String>,
}

#[cfg(windows)]
pub fn run() -> Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    let mut cfg = crate::config::load()?.taskbar_pin;
    while !crate::shutting_down() {
        let result = (|| -> Result<()> {
            cfg = crate::config::load()?.taskbar_pin;
            if cfg.enabled && !crate::pins_paused() {
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
    let cfg = crate::config::load()?.taskbar_pin;
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
fn tick(
    automation: &taskbar_kit::shell::Automation,
    cfg: &crate::config::TaskbarPin,
) -> Result<bool> {
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
