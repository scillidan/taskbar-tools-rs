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
    BeginPaint, CreateBitmap, CreateDIBSection, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
    GetDC, GetPixel, InvalidateRect, ReleaseDC, BITMAPINFO, BI_RGB, CLR_INVALID, DIB_RGB_COLORS,
    PAINTSTRUCT,
};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(windows)]
const APP: &str = "start-button";

#[cfg(windows)]
#[derive(Clone, Copy)]
struct Overlay {
    target: usize,
    window: usize,
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
    #[serde(default = "taskbar_kit::config::default_enabled")]
    enabled: bool,
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
pub fn inspect(automation: &taskbar_kit::shell::Automation) -> Result<()> {
    for (bar, rect) in taskbar_kit::shell::start_buttons(automation)? {
        println!(
            "taskbar {:?} secondary={} start={rect}",
            bar.hwnd, bar.secondary
        );
    }
    Ok(())
}

#[cfg(windows)]
pub fn run() -> Result<()> {
    let config_path = taskbar_kit::config::exe_dir_file("start-button.toml")?;
    let mut cfg = load_config(&config_path)?;
    while !cfg.enabled && !crate::shutting_down() {
        crate::wait_ms(500);
        cfg = load_config(&config_path)?;
    }
    if crate::shutting_down() {
        return Ok(());
    }
    let automation = taskbar_kit::shell::Automation::new()?;
    let icons = load_icons(&cfg)?;
    register_overlay_class()?;
    UI_THREAD.store(
        unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
        std::sync::atomic::Ordering::Relaxed,
    );

    sync(&automation, &icons)?;
    let timer = unsafe { SetTimer(None, 1, cfg.poll_ms.max(50), None) };
    if timer == 0 {
        anyhow::bail!("SetTimer failed");
    }
    taskbar_kit::message::run(None, |event| {
        let result = (|| -> Result<()> {
            match event {
                taskbar_kit::message::Event::Timer(1) => {
                    if !load_config(&config_path)?.enabled {
                        unsafe { PostQuitMessage(0) };
                        return Ok(());
                    }
                    sync(&automation, &icons)?;
                }
                taskbar_kit::message::Event::Wakeup(usize::MAX) => unsafe {
                    PostQuitMessage(0);
                },
                taskbar_kit::message::Event::Wakeup(1) => {
                    let clicks =
                        std::mem::take(&mut *CLICKS.lock().unwrap_or_else(|e| e.into_inner()));
                    for target in clicks {
                        taskbar_kit::shell::invoke_start(
                            &automation,
                            HWND(target as *mut core::ffi::c_void),
                        )?;
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            taskbar_kit::win::log(APP, &format!("{error:#}"));
        }
    });
    unsafe {
        let _ = KillTimer(None, timer);
    }
    destroy_overlays();
    UI_THREAD.store(0, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[cfg(windows)]
pub fn wake_for_shutdown() {
    let thread = UI_THREAD.load(std::sync::atomic::Ordering::Relaxed);
    if thread != 0 {
        let _ = taskbar_kit::win::post_thread_message(thread, usize::MAX, 0);
    }
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
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        return load_png_icon(&path, size).map(|icon| icon.0 as usize);
    }
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
fn load_png_icon(path: &Path, size: i32) -> Result<HICON> {
    let image = image::ImageReader::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?
        .decode()
        .with_context(|| format!("cannot decode {}", path.display()))?
        .resize_exact(
            size as u32,
            size as u32,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgba8();
    let mut bgra = Vec::with_capacity(image.as_raw().len());
    for pixel in image.pixels() {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = u32::try_from(std::mem::size_of_val(&info.bmiHeader))?;
    info.bmiHeader.biWidth = size;
    info.bmiHeader.biHeight = -size;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;
    unsafe {
        let dc = GetDC(None);
        let mut bits = core::ptr::null_mut();
        let color = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, dc);
        let color = color.context("CreateDIBSection failed")?;
        anyhow::ensure!(!bits.is_null(), "CreateDIBSection returned no pixels");
        core::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast(), bgra.len());
        let mask = CreateBitmap(size, size, 1, 1, None);
        anyhow::ensure!(!mask.0.is_null(), "CreateBitmap failed");
        let icon_info = windows::Win32::UI::WindowsAndMessaging::ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        };
        let icon = CreateIconIndirect(&icon_info);
        let _ = DeleteObject(mask.into());
        let _ = DeleteObject(color.into());
        icon.context("CreateIconIndirect failed")
    }
}

#[cfg(windows)]
fn sync(automation: &taskbar_kit::shell::Automation, icons: &Icons) -> Result<()> {
    let light = light_theme();
    let icon = if light { icons.light } else { icons.dark };
    let buttons = taskbar_kit::shell::start_buttons(automation)?;
    let active: HashSet<usize> = buttons.iter().map(|(bar, _)| bar.hwnd.0 as usize).collect();
    let stale: Vec<(usize, usize)> = OVERLAYS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|(key, _)| !active.contains(*key))
        .map(|(key, overlay)| (*key, overlay.window))
        .collect();
    for (key, window) in stale {
        unsafe {
            let _ = DestroyWindow(HWND(window as *mut core::ffi::c_void));
        }
        OVERLAYS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
    }
    for (bar, rect) in buttons {
        let key = bar.hwnd.0 as usize;
        let background = sample_background(bar.hwnd, light).0;
        let existing = OVERLAYS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .copied();
        let hwnd = match existing {
            Some(overlay) => HWND(overlay.window as *mut core::ffi::c_void),
            None => create_overlay(rect)?,
        };
        let changed = existing
            .map(|old| old.background != background || old.icon != icon)
            .unwrap_or(false);
        OVERLAYS.lock().unwrap_or_else(|e| e.into_inner()).insert(
            key,
            Overlay {
                target: key,
                window: hwnd.0 as usize,
                background,
                icon,
                icon_size: icons.size,
            },
        );
        if changed {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
        }
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
                .values()
                .find(|overlay| overlay.window == key)
                .map(|overlay| overlay.target);
            if let Some(target) = target {
                CLICKS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(target);
                let thread = UI_THREAD.load(std::sync::atomic::Ordering::Relaxed);
                let _ = taskbar_kit::win::post_thread_message(thread, 1, 0);
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
        .values()
        .find(|overlay| overlay.window == hwnd.0 as usize)
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
    for overlay in overlays.into_values() {
        unsafe {
            let _ = DestroyWindow(HWND(overlay.window as *mut core::ffi::c_void));
        }
    }
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

    #[cfg(windows)]
    #[test]
    fn png_becomes_an_icon() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/black_Lake.png"
        ));
        let icon = super::load_png_icon(path, 24).unwrap();
        assert!(!icon.0.is_null());
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
        }
    }
}
