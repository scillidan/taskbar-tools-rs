#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use serde::Deserialize;
#[cfg(windows)]
use std::collections::{HashMap, HashSet};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{LazyLock, Mutex};
#[cfg(windows)]
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetDC, GetPixel,
    InvalidateRect, ReleaseDC, CLR_INVALID, PAINTSTRUCT,
};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

const APP: &str = "start-button";

#[cfg(windows)]
#[derive(Clone, Copy)]
struct Overlay {
    target: usize,
    background: u32,
    icon: usize,
    icon_size: i32,
}

#[cfg(windows)]
static OVERLAYS: LazyLock<Mutex<HashMap<usize, Overlay>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
#[cfg(windows)]
static CLICKS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
#[cfg(windows)]
static UI_THREAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(windows)]
#[derive(Deserialize)]
struct FileConfig {
    #[serde(default = "default_poll")]
    poll_ms: u32,
    #[serde(default = "default_icon_size")]
    icon_size: i32,
    #[serde(default)]
    light_icon: String,
    #[serde(default)]
    dark_icon: String,
}

#[cfg(windows)]
fn default_poll() -> u32 {
    100
}
#[cfg(windows)]
fn default_icon_size() -> i32 {
    24
}

#[cfg(windows)]
#[derive(Default)]
struct Icons {
    light: usize,
    dark: usize,
    size: i32,
}

#[cfg(windows)]
impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.light, self.dark] {
            if icon != 0 {
                unsafe {
                    let _ = DestroyIcon(HICON(icon as *mut core::ffi::c_void));
                }
            }
        }
    }
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        taskbar_kit::win::attach_parent_console();
    }
    let automation = taskbar_kit::shell::Automation::new()?;
    if args.iter().any(|arg| arg == "--inspect") {
        for (bar, rect) in taskbar_kit::shell::start_buttons(&automation)? {
            println!(
                "taskbar {:?} secondary={} start={rect}",
                bar.hwnd, bar.secondary
            );
        }
        return Ok(());
    }
    taskbar_kit::win::hide_owned_console();
    unsafe {
        windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )?;
    }
    let _single = taskbar_kit::win::ensure_single_instance(APP)?;
    let config_path = taskbar_kit::config::exe_dir_file("start-button.toml")?;
    let cfg = load_config(&config_path)?;
    let icons = load_icons(&cfg)?;
    register_overlay_class()?;
    UI_THREAD.store(
        unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
        std::sync::atomic::Ordering::Relaxed,
    );

    let mut tray = taskbar_kit::tray::Tray::new(
        APP,
        taskbar_kit::icon::letter_rgba('s', [0x40, 0x90, 0xe0, 0xff], [0x20, 0x50, 0x80, 0xff]),
    )?;
    refresh_menu(&mut tray);
    sync(&automation, &icons)?;
    let timer = unsafe { SetTimer(None, 1, cfg.poll_ms.max(50), None) };
    if timer == 0 {
        anyhow::bail!("SetTimer failed");
    }
    tray.run(None, |tray, event| {
        let result = (|| -> Result<()> {
            match event {
                taskbar_kit::tray::Event::Timer(1) => sync(&automation, &icons)?,
                taskbar_kit::tray::Event::Wakeup(1) => {
                    let clicks =
                        std::mem::take(&mut *CLICKS.lock().unwrap_or_else(|e| e.into_inner()));
                    for target in clicks {
                        taskbar_kit::shell::invoke_start(
                            &automation,
                            HWND(target as *mut core::ffi::c_void),
                        )?;
                    }
                }
                taskbar_kit::tray::Event::Menu(label) if label == "Edit config" => {
                    taskbar_kit::win::open_with_default(&config_path);
                }
                taskbar_kit::tray::Event::Menu(label) if label == "Start with Windows" => {
                    let enabled = !taskbar_kit::win::autostart_enabled(APP);
                    taskbar_kit::win::set_autostart(APP, enabled)?;
                    refresh_menu(tray);
                }
                taskbar_kit::tray::Event::Menu(label) if label == "Exit" => tray.quit(),
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            taskbar_kit::win::log(APP, &format!("{error:#}"));
            tray.set_tooltip(&format!("{APP}: error; see {APP}.log"));
        }
    });
    unsafe {
        let _ = KillTimer(None, timer);
    }
    destroy_overlays();
    Ok(())
}

#[cfg(windows)]
fn load_config(path: &Path) -> Result<FileConfig> {
    let text = taskbar_kit::config::load_or_create(path, include_str!("../start-button.toml"))?;
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

#[cfg(windows)]
fn load_icons(cfg: &FileConfig) -> Result<Icons> {
    let size = cfg.icon_size.clamp(16, 64);
    Ok(Icons {
        light: load_icon(&cfg.light_icon, size)?,
        dark: load_icon(&cfg.dark_icon, size)?,
        size,
    })
}

#[cfg(windows)]
fn load_icon(value: &str, size: i32) -> Result<usize> {
    if value.trim().is_empty() {
        return Ok(0);
    }
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        taskbar_kit::config::exe_dir()?.join(path)
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        LoadImageW(
            None,
            windows::core::PCWSTR(wide.as_ptr()),
            IMAGE_ICON,
            size,
            size,
            LR_LOADFROMFILE,
        )
    }
    .with_context(|| format!("cannot load {}", path.display()))?;
    Ok(handle.0 as usize)
}

#[cfg(windows)]
fn sync(automation: &taskbar_kit::shell::Automation, icons: &Icons) -> Result<()> {
    let light = light_theme();
    let icon = if light { icons.light } else { icons.dark };
    let buttons = taskbar_kit::shell::start_buttons(automation)?;
    let active: HashSet<usize> = buttons.iter().map(|(bar, _)| bar.hwnd.0 as usize).collect();
    let stale: Vec<usize> = OVERLAYS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|(key, _)| !active.contains(*key))
        .map(|(key, _)| *key)
        .collect();
    for key in stale {
        unsafe {
            let _ = DestroyWindow(HWND(key as *mut core::ffi::c_void));
        }
        OVERLAYS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
    }
    for (bar, rect) in buttons {
        let key = bar.hwnd.0 as usize;
        let background = sample_background(bar.hwnd, light).0;
        let overlay = Overlay {
            target: key,
            background,
            icon,
            icon_size: icons.size,
        };
        let mut overlays = OVERLAYS.lock().unwrap_or_else(|e| e.into_inner());
        let hwnd = if let Some(existing) = overlays.get(&key).copied() {
            let changed = existing.background != background || existing.icon != icon;
            overlays.insert(key, overlay);
            drop(overlays);
            if changed {
                unsafe {
                    let _ = InvalidateRect(Some(HWND(key as *mut core::ffi::c_void)), None, false);
                }
            }
            HWND(key as *mut core::ffi::c_void)
        } else {
            drop(overlays);
            let hwnd = create_overlay(rect)?;
            OVERLAYS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key, overlay);
            hwnd
        };
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn create_overlay(rect: taskbar_kit::shell::Rect) -> Result<HWND> {
    let instance: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            windows::core::w!("TaskbarToolsStartOverlay"),
            windows::core::w!(""),
            WS_POPUP,
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            None,
            None,
            Some(instance),
            None,
        )
        .context("CreateWindowExW start overlay failed")
    }
}

#[cfg(windows)]
fn register_overlay_class() -> Result<()> {
    let instance: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();
    let class = WNDCLASSW {
        hInstance: instance,
        lpszClassName: windows::core::w!("TaskbarToolsStartOverlay"),
        lpfnWndProc: Some(overlay_proc),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        ..Default::default()
    };
    unsafe {
        if RegisterClassW(&class) == 0 {
            anyhow::bail!("RegisterClassW failed");
        }
    }
    Ok(())
}

#[cfg(windows)]
unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_LBUTTONUP => {
            let key = hwnd.0 as usize;
            let target = OVERLAYS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
                .map(|o| o.target);
            if let Some(target) = target {
                CLICKS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(target);
                let thread = UI_THREAD.load(std::sync::atomic::Ordering::Relaxed);
                let _ = taskbar_kit::win::post_thread_message(thread, 0, 1);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_overlay(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
fn paint_overlay(hwnd: HWND) {
    let Some(overlay) = OVERLAYS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(hwnd.0 as usize))
        .copied()
    else {
        return;
    };
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let brush = CreateSolidBrush(COLORREF(overlay.background));
        FillRect(dc, &client, brush);
        let _ = DeleteObject(brush.into());
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        let size = overlay.icon_size.min(width).min(height);
        let x = (width - size) / 2;
        let y = (height - size) / 2;
        if overlay.icon != 0 {
            let _ = DrawIconEx(
                dc,
                x,
                y,
                HICON(overlay.icon as *mut core::ffi::c_void),
                size,
                size,
                0,
                None,
                DI_NORMAL,
            );
        } else {
            let dark = color_luminance(overlay.background) < 128;
            let foreground = if dark { 0x00ff_ffff } else { 0x0000_2020 };
            let logo = CreateSolidBrush(COLORREF(foreground));
            let gap = 2;
            let pane = (size - gap) / 2;
            for (px, py) in [
                (x, y),
                (x + pane + gap, y),
                (x, y + pane + gap),
                (x + pane + gap, y + pane + gap),
            ] {
                let rect = RECT {
                    left: px,
                    top: py,
                    right: px + pane,
                    bottom: py + pane,
                };
                FillRect(dc, &rect, logo);
            }
            let _ = DeleteObject(logo.into());
        }
        let _ = EndPaint(hwnd, &paint);
    }
}

#[cfg(windows)]
fn color_luminance(color: u32) -> u32 {
    let r = color & 0xff;
    let g = (color >> 8) & 0xff;
    let b = (color >> 16) & 0xff;
    (r * 299 + g * 587 + b * 114) / 1000
}

#[cfg(windows)]
fn sample_background(taskbar: HWND, light: bool) -> COLORREF {
    unsafe {
        let dc = GetDC(Some(taskbar));
        if !dc.0.is_null() {
            let color = GetPixel(dc, 2, 2);
            ReleaseDC(Some(taskbar), dc);
            if color.0 != CLR_INVALID {
                return color;
            }
        }
    }
    COLORREF(if light { 0x00f3_f3f3 } else { 0x0020_2020 })
}

#[cfg(windows)]
fn light_theme() -> bool {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            windows::core::w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut _ as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    status.0 == 0 && value != 0
}

#[cfg(windows)]
fn destroy_overlays() {
    let overlays = std::mem::take(&mut *OVERLAYS.lock().unwrap_or_else(|e| e.into_inner()));
    for (key, _) in overlays {
        unsafe {
            let _ = DestroyWindow(HWND(key as *mut core::ffi::c_void));
        }
    }
}

#[cfg(windows)]
fn refresh_menu(tray: &mut taskbar_kit::tray::Tray) {
    use taskbar_kit::tray::Entry;
    tray.set_menu(&[
        Entry::Item("Edit config"),
        Entry::Toggle(
            "Start with Windows",
            taskbar_kit::win::autostart_enabled(APP),
        ),
        Entry::Sep,
        Entry::Item("Exit"),
    ]);
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::color_luminance;

    #[cfg(windows)]
    #[test]
    fn luminance_distinguishes_themes() {
        assert!(color_luminance(0x00ff_ffff) > 200);
        assert!(color_luminance(0x0020_2020) < 80);
    }
}
