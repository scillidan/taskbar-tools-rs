mod center_taskbar;
mod hidden_taskbar;
mod start_button;
mod taskbar_pin;
#[cfg(windows)]
mod tray;
mod tray_pin;

const APP: &str = "taskbar-tools-rs";

#[cfg(windows)]
static SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
pub fn shutting_down() -> bool {
    SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(windows)]
pub fn wait_ms(mut milliseconds: u64) {
    while milliseconds > 0 && !shutting_down() {
        let step = milliseconds.min(50);
        std::thread::sleep(std::time::Duration::from_millis(step));
        milliseconds -= step;
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        taskbar_kit::win::log(APP, &format!("{error:#}"));
        taskbar_kit::win::message_box(APP, &format!("{error:#}"));
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() -> ! {
    taskbar_kit::unimplemented(APP)
}

#[cfg(windows)]
fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        taskbar_kit::win::attach_parent_console();
    }
    match args.first().map(String::as_str) {
        None => run_daemon(),
        Some("--dump") if args.len() == 1 => dump(),
        Some("--inspect") if args.len() == 1 => inspect(),
        Some("--once") if args.len() == 1 => once(),
        Some("--help" | "-h") if args.len() == 1 => {
            println!("usage: taskbar-tools-rs [--dump|--inspect|--once]");
            Ok(())
        }
        _ => anyhow::bail!("usage: taskbar-tools-rs [--dump|--inspect|--once]"),
    }
}

#[cfg(windows)]
fn dump() -> anyhow::Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    taskbar_pin::dump(&automation)?;
    tray_pin::dump(&automation)
}

#[cfg(windows)]
fn inspect() -> anyhow::Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    taskbar_pin::inspect(&automation)?;
    center_taskbar::inspect()?;
    start_button::inspect(&automation)
}

#[cfg(windows)]
fn once() -> anyhow::Result<()> {
    let automation = taskbar_kit::shell::Automation::new()?;
    taskbar_pin::once(&automation)?;
    tray_pin::once(&automation)
}

#[cfg(windows)]
fn run_daemon() -> anyhow::Result<()> {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use windows::Win32::UI::WindowsAndMessaging::{KillTimer, PostQuitMessage, SetTimer};

    let _instance = taskbar_kit::win::ensure_single_instance(APP)?;
    taskbar_kit::win::hide_owned_console();
    // Process-wide: module threads must never race to set this again.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };
    let tray = tray::Tray::new()?;
    let timer = unsafe { SetTimer(None, 1, 100, None) };
    if timer == 0 {
        anyhow::bail!("SetTimer failed");
    }
    let handles = [
        spawn("taskbar-pin", taskbar_pin::run),
        spawn("tray-pin", tray_pin::run),
        spawn("center-taskbar", center_taskbar::run),
        spawn("start-button", start_button::run),
        spawn("hidden-taskbar", hidden_taskbar::run),
    ]
    .into_iter()
    .collect::<anyhow::Result<Vec<_>>>()?;

    taskbar_kit::message::run(None, |event| {
        if !matches!(event, taskbar_kit::message::Event::Timer(1)) {
            return;
        }
        while let Some(command) = tray.poll() {
            let result = match command {
                tray::Cmd::Edit(file) => tray::edit_config(file),
                tray::Cmd::TestTaskbar => taskbar_kit::shell::Automation::new()
                    .and_then(|automation| taskbar_pin::once(&automation)),
                tray::Cmd::TestTray => taskbar_kit::shell::Automation::new()
                    .and_then(|automation| tray_pin::once(&automation)),
                tray::Cmd::Quit => {
                    SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
                    start_button::wake_for_shutdown();
                    hidden_taskbar::wake_for_shutdown();
                    unsafe { PostQuitMessage(0) };
                    return;
                }
            };
            if let Err(error) = result {
                taskbar_kit::win::message_box(APP, &format!("{error:#}"));
            }
        }
    });
    unsafe {
        let _ = KillTimer(None, timer);
    }
    drop(tray);
    for handle in handles {
        let _ = handle.join();
    }
    Ok(())
}

#[cfg(windows)]
fn spawn(
    name: &'static str,
    run: fn() -> anyhow::Result<()>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            while !shutting_down() {
                if let Err(error) = run() {
                    taskbar_kit::win::log(name, &format!("{error:#}"));
                }
                wait_ms(1000);
            }
        })
        .map_err(Into::into)
}
