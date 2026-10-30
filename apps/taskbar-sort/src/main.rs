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
use taskbar_kit::sorting::{drag_move, first_move, taskbar_groups};

const APP: &str = "taskbar-sort";

#[cfg(windows)]
#[derive(Deserialize)]
struct FileConfig {
    #[serde(default = "default_poll")]
    poll_ms: u32,
    #[serde(default = "default_delay")]
    delay_ms: u64,
    #[serde(default = "default_apply")]
    apply: bool,
    #[serde(default)]
    order: OrderConfig,
}

#[cfg(windows)]
fn default_poll() -> u32 {
    750
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
#[derive(Default, Deserialize, Serialize)]
struct Cache {
    #[serde(default)]
    order: Vec<String>,
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
        let groups = taskbar_groups(&automation)?;
        let cache = Cache {
            order: groups.first().map(|group| group.ids()).unwrap_or_default(),
        };
        print!("{}", toml::to_string_pretty(&cache)?);
        return Ok(());
    }

    taskbar_kit::win::hide_owned_console();
    let _single = taskbar_kit::win::ensure_single_instance(APP)?;
    let config_path = taskbar_kit::config::exe_dir_file("taskbar-sort.toml")?;
    let cache_path = taskbar_kit::config::exe_dir_file("taskbar-sort-custom.toml")?;
    let mut config_modified = None;
    let mut cfg = load_config(&config_path, &mut config_modified)?;
    let mut cache = load_cache(&cache_path)?;
    let mut states: HashMap<String, GroupState> = HashMap::new();
    let mut apply = cfg.apply;

    if args.iter().any(|arg| arg == "--once") {
        for _ in 0..100 {
            tick(
                &automation,
                &cfg,
                apply,
                &mut states,
                &mut cache,
                &cache_path,
                true,
            )?;
            let pending = taskbar_groups(&automation)?.into_iter().any(|group| {
                let target =
                    planned_order(&group.order_items(&cfg.order), &cfg.order, &cache.order);
                first_move(&group.ids(), &target).is_some()
            });
            if !pending {
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
                        &mut cache,
                        &cache_path,
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
    Ok(())
}

#[cfg(windows)]
fn tick(
    automation: &taskbar_kit::shell::Automation,
    cfg: &FileConfig,
    apply: bool,
    states: &mut HashMap<String, GroupState>,
    cache: &mut Cache,
    cache_path: &Path,
    force: bool,
) -> Result<()> {
    let now = Instant::now();
    for group in taskbar_groups(automation)? {
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
            if cache.order != ids {
                cache.order = ids;
                save_cache(cache_path, cache)?;
            }
            continue;
        }
        if cfg.order.mode == SortMode::Custom && (!apply || cache.order.is_empty()) {
            cache.order = ids;
            save_cache(cache_path, cache)?;
            state.initialized = true;
            continue;
        }
        let target = state.target.clone().unwrap_or_else(|| {
            planned_order(&group.order_items(&cfg.order), &cfg.order, &cache.order)
        });
        if ids == target {
            state.initialized = true;
            state.target = None;
            if cfg.order.mode == SortMode::Custom && cache.order != ids {
                cache.order = ids;
                save_cache(cache_path, cache)?;
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
    Ok(())
}

#[cfg(windows)]
fn load_config(path: &Path, modified: &mut Option<SystemTime>) -> Result<FileConfig> {
    let text = taskbar_kit::config::load_or_create(path, include_str!("../taskbar-sort.toml"))?;
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
fn load_cache(path: &Path) -> Result<Cache> {
    if !path.exists() {
        return Ok(Cache::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

#[cfg(windows)]
fn save_cache(path: &Path, cache: &Cache) -> Result<()> {
    taskbar_kit::config::write_atomic(path, &toml::to_string_pretty(cache)?)
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
