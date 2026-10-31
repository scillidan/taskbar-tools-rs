#[cfg(any(windows, test))]
mod geometry;
#[cfg(windows)]
mod platform;

#[cfg(windows)]
const APP: &str = "hidden-taskbar";
#[cfg(windows)]
static THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(windows)]
pub fn run() -> anyhow::Result<()> {
    use taskbar_kit::keys::Combo;
    use taskbar_kit::message::Event;
    use taskbar_kit::{config, hotkey::Hotkeys};
    use windows::Win32::UI::WindowsAndMessaging::{KillTimer, PostQuitMessage, SetTimer};

    #[derive(serde::Deserialize)]
    struct General {
        enabled: Option<bool>,
        hotkey: Option<String>,
        resize_snapped: Option<bool>,
    }
    #[derive(serde::Deserialize, Default)]
    struct FileConfig {
        general: Option<General>,
    }

    let config_path = config::exe_dir_file("hidden-taskbar.toml")?;
    let read_config = || -> anyhow::Result<(bool, Combo, bool)> {
        let text = config::load_or_create(&config_path, include_str!("../hidden-taskbar.toml"))?;
        let file: FileConfig = toml::from_str(&text)?;
        let general = file.general;
        let enabled = general.as_ref().and_then(|g| g.enabled).unwrap_or(true);
        let combo = general
            .as_ref()
            .and_then(|g| g.hotkey.as_deref())
            .unwrap_or("ctrl+alt+f9")
            .parse()?;
        let resize = general.and_then(|g| g.resize_snapped).unwrap_or(true);
        Ok((enabled, combo, resize))
    };
    while !crate::shutting_down() {
        let (enabled, combo, resize_snapped) = read_config()?;
        if !enabled {
            crate::wait_ms(500);
            continue;
        }
        THREAD_ID.store(
            unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
            std::sync::atomic::Ordering::Relaxed,
        );
        let hotkeys = Hotkeys::register(&[("toggle", combo)])?;
        let mut app = platform::App::new(resize_snapped);
        let timer = unsafe { SetTimer(None, 1, 500, None) };
        if timer == 0 {
            anyhow::bail!("SetTimer failed");
        }
        taskbar_kit::message::run(Some(&hotkeys), |event| match event {
            Event::Wakeup(usize::MAX) => unsafe { PostQuitMessage(0) },
            Event::Hotkey(label) if label == "toggle" => {
                report_result(app.toggle_foreground());
            }
            Event::Timer(1) => {
                let result = read_config();
                match result {
                    Ok((enabled, new_combo, new_resize))
                        if !enabled || new_combo != combo || new_resize != resize_snapped =>
                    {
                        unsafe { PostQuitMessage(0) };
                    }
                    Ok(_) => {}
                    Err(error) => taskbar_kit::win::log(APP, &format!("{error:#}")),
                }
            }
            _ => {}
        });
        unsafe {
            let _ = KillTimer(None, timer);
        }
        report_warnings(app.restore_all());
        THREAD_ID.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(windows)]
pub fn wake_for_shutdown() {
    let thread = THREAD_ID.load(std::sync::atomic::Ordering::Relaxed);
    if thread != 0 {
        let _ = taskbar_kit::win::post_thread_message(thread, usize::MAX, 0);
    }
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
