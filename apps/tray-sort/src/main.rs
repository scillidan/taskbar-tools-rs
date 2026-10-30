const APP: &str = "tray-sort";

#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::collections::HashMap;
#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::time::{Duration, Instant, SystemTime};
#[cfg(windows)]
use taskbar_kit::order::{planned_order, OrderConfig, SortMode};
#[cfg(windows)]
use taskbar_kit::sorting::{drag_move, first_move, tray_groups, TrayArea};

#[cfg(windows)]
#[derive(Deserialize)]
struct FileConfig {
    #[serde(default = "default_poll")]
    poll_ms: u32,
    #[serde(default = "default_apply")]
    apply: bool,
    #[serde(default)]
    visible: AreaConfig,
    #[serde(default)]
    hidden: AreaConfig,
}

#[cfg(windows)]
#[derive(Default, Deserialize)]
struct AreaConfig {
    #[serde(flatten)]
    order: OrderConfig,
    #[serde(default = "default_delay")]
    delay_ms: u64,
}

#[cfg(windows)]
fn default_poll() -> u32 {
    600
}
#[cfg(windows)]
fn default_delay() -> u64 {
    1500
}
#[cfg(windows)]
fn default_apply() -> bool {
    true
}

#[cfg(windows)]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
struct CustomState {
    #[serde(default)]
    visible: Vec<String>,
    #[serde(default)]
    hidden: Vec<String>,
}

#[cfg(windows)]
#[derive(Default)]
struct GroupState {
    last: Vec<String>,
    changed: Option<Instant>,
    initialized: bool,
    target: Option<Vec<String>>,
    cooldown: Option<Instant>,
}

#[cfg(windows)]
fn main() -> Result<()> {
    if let Err(error) = run() {
        taskbar_kit::win::log(APP, &format!("{error:#}"));
        taskbar_kit::win::message_box(APP, &format!("{error:#}"));
        return Err(error);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    taskbar_kit::unimplemented(APP)
}

#[cfg(windows)]
fn run() -> Result<()> {
    use taskbar_kit::shell::Automation;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        taskbar_kit::win::attach_parent_console();
    }
    let automation = Automation::new()?;
    if args.iter().any(|arg| arg == "--inspect") {
        inspect(&automation)?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--dump") {
        let state = CustomState {
            visible: dump_area(&automation, TrayArea::Visible)?,
            hidden: dump_area(&automation, TrayArea::Hidden)?,
        };
        print!("{}", toml::to_string_pretty(&state)?);
        return Ok(());
    }

    taskbar_kit::win::hide_owned_console();
    let _single = taskbar_kit::win::ensure_single_instance(APP)?;
    let config_path = taskbar_kit::config::exe_dir_file("tray-sort.toml")?;
    let custom_path = taskbar_kit::config::exe_dir_file("tray-sort-custom.toml")?;
    let mut config_modified = None;
    let mut cfg = load_config(&config_path, &mut config_modified)?;
    let mut custom = load_custom(&custom_path)?;
    let mut states: HashMap<String, GroupState> = HashMap::new();
    let mut apply = cfg.apply;

    if args.iter().any(|arg| arg == "--once") {
        for _ in 0..100 {
            tick(
                &automation,
                &cfg,
                apply,
                &mut states,
                &mut custom,
                &custom_path,
                true,
            )?;
            if !pending(&automation, &cfg, &custom)? {
                break;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        return Ok(());
    }

    use taskbar_kit::tray::{Event, Tray};
    let mut tray = Tray::new(
        APP,
        taskbar_kit::icon::letter_rgba('s', [0x60, 0x50, 0xa0, 0xff], [0x38, 0x30, 0x60, 0xff]),
    )?;
    refresh_menu(&mut tray, apply);
    let timer = unsafe {
        windows::Win32::UI::WindowsAndMessaging::SetTimer(None, 1, cfg.poll_ms.max(100), None)
    };
    if timer == 0 {
        anyhow::bail!("SetTimer failed");
    }
    let mut force = true;
    tray.run(None, |tray, event| {
        let result = (|| -> Result<()> {
            match event {
                Event::Timer(1) => {
                    let previous = config_modified;
                    reload_if_changed(&config_path, &mut config_modified, &mut cfg)?;
                    if previous != config_modified {
                        states.clear();
                        force = true;
                    }
                    tick(
                        &automation,
                        &cfg,
                        apply,
                        &mut states,
                        &mut custom,
                        &custom_path,
                        force,
                    )?;
                    force = false;
                    Ok(())
                }
                Event::Menu(label) if label == "Sort now" => {
                    force = true;
                    states.clear();
                    Ok(())
                }
                Event::Menu(label) if label == "Apply sorting" => {
                    apply = !apply;
                    refresh_menu(tray, apply);
                    Ok(())
                }
                Event::Menu(label) if label == "Edit config" => {
                    taskbar_kit::win::open_with_default(&config_path);
                    Ok(())
                }
                Event::Menu(label) if label == "Start with Windows" => {
                    let enabled = !taskbar_kit::win::autostart_enabled(APP);
                    taskbar_kit::win::set_autostart(APP, enabled)?;
                    refresh_menu(tray, apply);
                    Ok(())
                }
                Event::Menu(label) if label == "Exit" => {
                    tray.quit();
                    Ok(())
                }
                _ => Ok(()),
            }
        })();
        if let Err(error) = result {
            taskbar_kit::win::log(APP, &format!("{error:#}"));
            tray.set_tooltip(&format!("{APP}: error; see {APP}.log"));
        }
    });
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(None, timer);
    }
    Ok(())
}

#[cfg(windows)]
fn inspect(automation: &taskbar_kit::shell::Automation) -> Result<()> {
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
    for window in taskbar_kit::shell::overflow_windows()? {
        println!(
            "overflow {:?} class={:?} visible={} rect={}",
            window.hwnd, window.class_name, window.visible, window.rect
        );
        if window.visible {
            for item in automation.descendants_of(window.hwnd)? {
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
    }
    Ok(())
}

#[cfg(windows)]
fn dump_area(automation: &taskbar_kit::shell::Automation, area: TrayArea) -> Result<Vec<String>> {
    Ok(tray_groups(automation, area)?
        .into_iter()
        .flat_map(|group| group.ids())
        .collect())
}

#[cfg(windows)]
fn pending(
    automation: &taskbar_kit::shell::Automation,
    cfg: &FileConfig,
    custom: &CustomState,
) -> Result<bool> {
    for (area, area_cfg, order) in [
        (TrayArea::Visible, &cfg.visible, &custom.visible),
        (TrayArea::Hidden, &cfg.hidden, &custom.hidden),
    ] {
        for group in tray_groups(automation, area)? {
            let target = planned_order(&group.order_items(&area_cfg.order), &area_cfg.order, order);
            if first_move(&group.ids(), &target).is_some() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(windows)]
fn tick(
    automation: &taskbar_kit::shell::Automation,
    cfg: &FileConfig,
    apply: bool,
    states: &mut HashMap<String, GroupState>,
    custom: &mut CustomState,
    custom_path: &Path,
    force: bool,
) -> Result<()> {
    if tick_area(
        automation,
        TrayArea::Visible,
        &cfg.visible,
        apply,
        states,
        &mut custom.visible,
        force,
    )? {
        save_custom(custom_path, custom)?;
    }
    if tick_area(
        automation,
        TrayArea::Hidden,
        &cfg.hidden,
        apply,
        states,
        &mut custom.hidden,
        force,
    )? {
        save_custom(custom_path, custom)?;
    }
    Ok(())
}

#[cfg(windows)]
fn tick_area(
    automation: &taskbar_kit::shell::Automation,
    area: TrayArea,
    cfg: &AreaConfig,
    apply: bool,
    states: &mut HashMap<String, GroupState>,
    custom: &mut Vec<String>,
    force: bool,
) -> Result<bool> {
    let now = Instant::now();
    let mut custom_changed = false;
    for group in tray_groups(automation, area)? {
        if group.items.is_empty() {
            continue;
        }
        let ids = group.ids();
        let state = states
            .entry(group.key.clone())
            .or_insert_with(|| GroupState {
                changed: Some(now),
                ..Default::default()
            });
        if state.last != ids {
            state.last = ids.clone();
            state.changed = Some(now);
        }
        if !force
            && state.changed.is_some_and(|changed| {
                now.duration_since(changed) < Duration::from_millis(cfg.delay_ms)
            })
        {
            continue;
        }
        if cfg.order.mode == SortMode::Custom && state.initialized && state.target.is_none() {
            if *custom != ids {
                *custom = ids;
                custom_changed = true;
            }
            continue;
        }
        if cfg.order.mode == SortMode::Custom && (!apply || custom.is_empty()) {
            *custom = ids;
            custom_changed = true;
            state.initialized = true;
            continue;
        }
        let target = state
            .target
            .clone()
            .unwrap_or_else(|| planned_order(&group.order_items(&cfg.order), &cfg.order, custom));
        if ids == target {
            state.initialized = true;
            state.target = None;
            if cfg.order.mode == SortMode::Custom && *custom != ids {
                *custom = ids;
                custom_changed = true;
            }
            continue;
        }
        if apply && state.cooldown.is_none_or(|until| now >= until) {
            if let Some((id, index)) = first_move(&ids, &target) {
                drag_move(&group, &id, index)
                    .with_context(|| format!("moving {id} in {}", group.key))?;
                state.cooldown = Some(now + Duration::from_millis(600));
                if cfg.order.mode == SortMode::Custom {
                    state.target = Some(target);
                }
            }
        }
    }
    Ok(custom_changed)
}

#[cfg(windows)]
fn load_config(path: &Path, modified: &mut Option<SystemTime>) -> Result<FileConfig> {
    let text = taskbar_kit::config::load_or_create(path, include_str!("../tray-sort.toml"))?;
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

#[cfg(windows)]
fn load_custom(path: &Path) -> Result<CustomState> {
    if !path.exists() {
        return Ok(CustomState::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

#[cfg(windows)]
fn save_custom(path: &Path, custom: &CustomState) -> Result<()> {
    taskbar_kit::config::write_atomic(path, &toml::to_string_pretty(custom)?)
}

#[cfg(windows)]
fn refresh_menu(tray: &mut taskbar_kit::tray::Tray, apply: bool) {
    use taskbar_kit::tray::Entry;
    tray.set_menu(&[
        Entry::Item("Sort now"),
        Entry::Toggle("Apply sorting", apply),
        Entry::Item("Edit config"),
        Entry::Toggle(
            "Start with Windows",
            taskbar_kit::win::autostart_enabled(APP),
        ),
        Entry::Sep,
        Entry::Item("Exit"),
    ]);
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_independent_areas() {
        let cfg: FileConfig = toml::from_str(include_str!("../tray-sort.toml")).unwrap();
        assert!(cfg.apply);
        assert_eq!(cfg.poll_ms, 600);
        assert_eq!(cfg.visible.delay_ms, 1500);
        assert_eq!(cfg.hidden.delay_ms, 3000);
        assert_eq!(cfg.visible.order.mode, SortMode::Alphabetical);
        assert_eq!(cfg.hidden.order.mode, SortMode::Alphabetical);
    }

    #[test]
    fn custom_state_toml_is_loadable() {
        let state = CustomState {
            visible: vec!["visible-id".into()],
            hidden: vec!["hidden-id".into()],
        };
        let text = toml::to_string_pretty(&state).unwrap();
        assert_eq!(toml::from_str::<CustomState>(&text).unwrap(), state);
    }
}
