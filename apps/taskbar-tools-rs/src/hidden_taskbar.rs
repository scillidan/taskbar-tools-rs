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
    use taskbar_kit::hotkey::Hotkeys;
    use taskbar_kit::keys::Combo;
    use taskbar_kit::message::Event;
    use windows::Win32::UI::WindowsAndMessaging::{KillTimer, PostQuitMessage, SetTimer};

    while !crate::shutting_down() {
        let cfg = crate::config::load()?.hidden_taskbar;
        let enabled = cfg.enabled;
        let combo: Combo = cfg.hotkey.parse()?;
        let resize_snapped = cfg.resize_snapped;
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
            Event::Hotkey(label) if label == "toggle" && !crate::hotkeys_suspended() => {
                report_result(app.toggle_foreground());
            }
            Event::Timer(1) => {
                let result = (|| -> anyhow::Result<()> {
                    let cfg = crate::config::load()?.hidden_taskbar;
                    let new_combo: Combo = cfg.hotkey.parse()?;
                    if !cfg.enabled || new_combo != combo || cfg.resize_snapped != resize_snapped {
                        unsafe { PostQuitMessage(0) };
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    taskbar_kit::win::log(APP, &format!("{error:#}"));
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
