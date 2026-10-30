#[cfg(any(windows, test))]
mod geometry;
#[cfg(windows)]
mod platform;

const APP: &str = "hidden-taskbar";

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        if let Err(e) = run() {
            report(&format!("Error:\n{e:#}"));
            return Err(e);
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        taskbar_kit::unimplemented(APP)
    }
}

#[cfg(windows)]
fn run() -> anyhow::Result<()> {
    use taskbar_kit::keys::Combo;
    use taskbar_kit::tray::{Event, Tray};
    use taskbar_kit::{config, hotkey::Hotkeys, win};

    #[derive(serde::Deserialize)]
    struct General {
        hotkey: Option<String>,
        resize_snapped: Option<bool>,
    }
    #[derive(serde::Deserialize, Default)]
    struct FileConfig {
        general: Option<General>,
    }

    unsafe {
        windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )?;
    }
    win::hide_owned_console();
    let _single = win::ensure_single_instance(APP)?;
    let config_path = config::exe_dir_file("hidden-taskbar.toml")?;
    let text = config::load_or_create(&config_path, include_str!("../hidden-taskbar.toml"))?;
    let file: FileConfig = toml::from_str(&text)?;
    let general = file.general;
    let combo: Combo = general
        .as_ref()
        .and_then(|g| g.hotkey.as_deref())
        .unwrap_or("ctrl+alt+f9")
        .parse()?;
    let resize_snapped = general.and_then(|g| g.resize_snapped).unwrap_or(true);
    let hotkeys = Hotkeys::register(&[("toggle", combo)])?;
    let mut app = platform::App::new(resize_snapped);
    let mut tray = Tray::new(
        APP,
        taskbar_kit::icon::letter_rgba('t', [0x3b, 0x6e, 0xa5, 0xff], [0x23, 0x46, 0x6e, 0xff]),
    )?;
    refresh_menu(&mut tray);

    tray.run(Some(&hotkeys), |tray, event| match event {
        Event::Hotkey(label) if label == "toggle" => report_result(app.toggle_foreground()),
        Event::Menu(label) if label == "Toggle foreground" => {
            report_result(app.toggle_foreground());
        }
        Event::Menu(label) if label == "Restore all" => report_warnings(app.restore_all()),
        Event::Menu(label) if label == "Edit config" => win::open_with_default(&config_path),
        Event::Menu(label) if label == "Start with Windows" => {
            let now = !win::autostart_enabled(APP);
            if let Err(e) = win::set_autostart(APP, now) {
                report(&format!("Autostart failed:\n{e:#}"));
            }
            refresh_menu(tray);
        }
        Event::Menu(label) if label == "Exit" => tray.quit(),
        _ => {}
    });
    report_warnings(app.restore_all());
    Ok(())
}

#[cfg(windows)]
fn refresh_menu(tray: &mut taskbar_kit::tray::Tray) {
    use taskbar_kit::tray::Entry;
    tray.set_menu(&[
        Entry::Item("Toggle foreground"),
        Entry::Item("Restore all"),
        Entry::Sep,
        Entry::Item("Edit config"),
        Entry::Toggle(
            "Start with Windows",
            taskbar_kit::win::autostart_enabled(APP),
        ),
        Entry::Sep,
        Entry::Item("Exit"),
    ]);
}

#[cfg(windows)]
fn report_result(result: anyhow::Result<Vec<String>>) {
    match result {
        Ok(warnings) => report_warnings(warnings),
        Err(e) => report(&format!("Operation failed:\n{e:#}")),
    }
}

#[cfg(windows)]
fn report_warnings(warnings: Vec<String>) {
    if !warnings.is_empty() {
        report(&format!(
            "Some changes could not be applied:\n{}",
            warnings.join("\n")
        ));
    }
}

#[cfg(windows)]
fn report(text: &str) {
    eprintln!("{APP}: {text}");
    taskbar_kit::win::message_box(APP, text);
}
