use super::geometry::{extend_snapped, selectable, Rect};
use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTONULL,
};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy)]
struct Taskbar {
    hwnd: HWND,
    rect: Rect,
    monitor: HMONITOR,
    monitor_rect: Rect,
}

struct SavedWindow {
    hwnd: HWND,
    original: Rect,
    extended: Rect,
}

struct HiddenTaskbar {
    hwnd: HWND,
    windows: Vec<SavedWindow>,
}

pub struct App {
    hidden: Vec<HiddenTaskbar>,
    resize_snapped: bool,
}

impl App {
    pub fn new(resize_snapped: bool) -> Self {
        Self {
            hidden: Vec::new(),
            resize_snapped,
        }
    }

    pub fn toggle_foreground(&mut self) -> Result<Vec<String>> {
        let foreground = taskbar_kit::win::foreground_hwnd().context("no foreground window")?;
        let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
        if monitor.0.is_null() {
            anyhow::bail!("cannot locate the foreground window's monitor");
        }
        let taskbar = taskbars()
            .into_iter()
            .find(|taskbar| {
                let taskbar_monitor =
                    unsafe { MonitorFromWindow(taskbar.hwnd, MONITOR_DEFAULTTONULL) };
                taskbar_monitor == monitor
            })
            .context("no Explorer taskbar on the foreground window's monitor")?;

        if let Some(index) = self.hidden.iter().position(|h| h.hwnd == taskbar.hwnd) {
            return Ok(restore(self.hidden.remove(index)));
        }
        if unsafe { !IsWindowVisible(taskbar.hwnd).as_bool() } {
            let _ = unsafe { ShowWindow(taskbar.hwnd, SW_SHOW) };
            return Ok(Vec::new());
        }

        let (hidden, warnings) = self.hide(taskbar)?;
        self.hidden.push(hidden);
        Ok(warnings)
    }

    fn hide(&self, taskbar: Taskbar) -> Result<(HiddenTaskbar, Vec<String>)> {
        let (windows, mut warnings) = if self.resize_snapped {
            snapped_windows(taskbar)?
        } else {
            (Vec::new(), Vec::new())
        };
        let _ = unsafe { ShowWindow(taskbar.hwnd, SW_HIDE) };
        for window in &windows {
            if let Err(e) = set_rect(window.hwnd, window.extended) {
                warnings.push(format!("resize {:?}: {e:#}", window.hwnd));
            }
        }
        Ok((
            HiddenTaskbar {
                hwnd: taskbar.hwnd,
                windows,
            },
            warnings,
        ))
    }

    pub fn restore_all(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        for hidden in std::mem::take(&mut self.hidden) {
            warnings.extend(restore(hidden));
        }
        warnings
    }
}

fn taskbars() -> Vec<Taskbar> {
    [w!("Shell_TrayWnd"), w!("Shell_SecondaryTrayWnd")]
        .into_iter()
        .filter_map(|class| unsafe { FindWindowW(class, None) }.ok())
        .filter(|hwnd| !hwnd.0.is_null())
        .filter_map(|hwnd| {
            let rect = window_rect(hwnd).ok()?;
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
            if monitor.0.is_null() {
                return None;
            }
            let monitor_rect = monitor_rect(monitor).ok()?;
            Some(Taskbar {
                hwnd,
                rect,
                monitor,
                monitor_rect,
            })
        })
        .collect()
}

struct Collect {
    taskbar: Taskbar,
    windows: Vec<SavedWindow>,
    warnings: Vec<String>,
}

fn snapped_windows(taskbar: Taskbar) -> Result<(Vec<SavedWindow>, Vec<String>)> {
    let mut collect = Collect {
        taskbar,
        windows: Vec::new(),
        warnings: Vec::new(),
    };
    unsafe {
        EnumWindows(
            Some(enum_window),
            LPARAM(&mut collect as *mut Collect as isize),
        )?;
    }
    Ok((collect.windows, collect.warnings))
}

unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let collect = unsafe { &mut *(lparam.0 as *mut Collect) };
    match saved_window(hwnd, collect.taskbar) {
        Ok(Some(window)) => collect.windows.push(window),
        Ok(None) => {}
        Err(e) => collect.warnings.push(format!("inspect {:?}: {e:#}", hwnd)),
    }
    BOOL(1)
}

fn saved_window(hwnd: HWND, taskbar: Taskbar) -> Result<Option<SavedWindow>> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
    if monitor != taskbar.monitor || !eligible(hwnd)? {
        return Ok(None);
    }
    let original = window_rect(hwnd)?;
    let frame = visible_rect(hwnd).unwrap_or(original);
    Ok(
        extend_snapped(original, frame, taskbar.rect, taskbar.monitor_rect).map(|extended| {
            SavedWindow {
                hwnd,
                original,
                extended,
            }
        }),
    )
}

fn visible_rect(hwnd: HWND) -> Result<Rect> {
    let mut rect = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )?;
    }
    Ok(Rect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

fn eligible(hwnd: HWND) -> Result<bool> {
    if unsafe { !IsWindow(Some(hwnd)).as_bool() } {
        return Ok(false);
    }
    let class = class_name(hwnd)?;
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    Ok(selectable(
        unsafe { IsWindowVisible(hwnd).as_bool() },
        unsafe { IsIconic(hwnd).as_bool() },
        unsafe { IsZoomed(hwnd).as_bool() },
        ex_style & WS_EX_TOOLWINDOW.0 as isize != 0,
        &class,
    ))
}

fn restore(hidden: HiddenTaskbar) -> Vec<String> {
    let mut warnings = Vec::new();
    for window in hidden.windows {
        if unsafe { !IsWindow(Some(window.hwnd)).as_bool() } {
            continue;
        }
        match eligible(window.hwnd) {
            Ok(true) => {
                if let Err(e) = set_rect(window.hwnd, window.original) {
                    warnings.push(format!("restore {:?}: {e:#}", window.hwnd));
                }
            }
            Ok(false) => {}
            Err(e) => warnings.push(format!("inspect {:?}: {e:#}", window.hwnd)),
        }
    }
    if unsafe { IsWindow(Some(hidden.hwnd)).as_bool() } {
        let _ = unsafe { ShowWindow(hidden.hwnd, SW_SHOW) };
    } else {
        warnings.push(format!("taskbar {:?} no longer exists", hidden.hwnd));
    }
    warnings
}

fn class_name(hwnd: HWND) -> Result<String> {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len == 0 {
        return Err(windows::core::Error::from_win32().into());
    }
    Ok(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn window_rect(hwnd: HWND) -> Result<Rect> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect)? };
    Ok(Rect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

fn monitor_rect(monitor: HMONITOR) -> Result<Rect> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info).ok()? };
    Ok(Rect {
        left: info.rcMonitor.left,
        top: info.rcMonitor.top,
        right: info.rcMonitor.right,
        bottom: info.rcMonitor.bottom,
    })
}

fn set_rect(hwnd: HWND, rect: Rect) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
    }
    Ok(())
}
