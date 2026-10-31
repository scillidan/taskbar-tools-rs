//! Read-only Explorer window and UI Automation discovery.

use anyhow::{Context, Result};
use std::fmt;
use std::sync::{Mutex, MutexGuard};
use windows::core::BSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, TreeScope_Children, TreeScope_Descendants,
    UIA_ButtonControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetWindowRect};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
    pub fn center(self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
    pub fn is_empty(self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<Rect> for RECT {
    fn from(r: Rect) -> Self {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{} {}x{}",
            self.left,
            self.top,
            self.width(),
            self.height()
        )
    }
}

#[derive(Clone, Debug)]
pub struct UiItem {
    pub name: String,
    pub automation_id: String,
    pub class_name: String,
    pub control_type: i32,
    pub offscreen: bool,
    pub rect: Rect,
}

impl UiItem {
    pub fn identity(&self) -> String {
        if self.automation_id.is_empty() {
            self.name.clone()
        } else {
            format!("{}|{}", self.automation_id, self.name)
        }
    }

    pub fn is_button(&self) -> bool {
        self.control_type == UIA_ButtonControlTypeId.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Taskbar {
    pub hwnd: HWND,
    pub secondary: bool,
    pub rect: Rect,
}

pub fn taskbars() -> Result<Vec<Taskbar>> {
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class);
        let class = String::from_utf16_lossy(&class[..len as usize]);
        let secondary = match class.as_str() {
            "Shell_TrayWnd" => false,
            "Shell_SecondaryTrayWnd" => true,
            _ => return true.into(),
        };
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let bars = &mut *(data.0 as *mut Vec<Taskbar>);
            bars.push(Taskbar {
                hwnd,
                secondary,
                rect: rect.into(),
            });
        }
        true.into()
    }
    let mut bars: Vec<Taskbar> = Vec::new();
    unsafe {
        EnumWindows(Some(visit), LPARAM(&mut bars as *mut _ as isize))
            .ok()
            .context("EnumWindows failed")?;
    }
    bars.sort_by_key(|bar| bar.secondary);
    Ok(bars)
}

// Explorer's legacy UIA provider can block when modules query it concurrently.
static UIA_LOCK: Mutex<()> = Mutex::new(());

pub fn lock_ui() -> MutexGuard<'static, ()> {
    UIA_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// Fields drop in declaration order: release UIA before uninitializing COM.
pub struct Automation {
    uia: IUIAutomation,
    _com: ComGuard,
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

impl Automation {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let com = ComGuard;
            let uia = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .context("cannot create UI Automation")?;
            Ok(Self { uia, _com: com })
        }
    }

    pub fn desktop_descendants(&self) -> Result<Vec<UiItem>> {
        let _lock = lock_ui();
        let root = unsafe { self.uia.GetRootElement()? };
        self.descendants(&root)
    }

    pub fn descendants_of(&self, hwnd: HWND) -> Result<Vec<UiItem>> {
        let _lock = lock_ui();
        let root = unsafe { self.uia.ElementFromHandle(hwnd)? };
        self.descendants(&root)
    }

    pub fn children_of(&self, hwnd: HWND) -> Result<Vec<UiItem>> {
        let _lock = lock_ui();
        let root = unsafe { self.uia.ElementFromHandle(hwnd)? };
        let condition = unsafe { self.uia.CreateTrueCondition()? };
        let array = unsafe { root.FindAll(TreeScope_Children, &condition)? };
        elements(&array)
    }

    fn descendants(&self, root: &IUIAutomationElement) -> Result<Vec<UiItem>> {
        let condition = unsafe { self.uia.CreateTrueCondition()? };
        let array = unsafe { root.FindAll(TreeScope_Descendants, &condition)? };
        elements(&array)
    }
}

fn elements(
    array: &windows::Win32::UI::Accessibility::IUIAutomationElementArray,
) -> Result<Vec<UiItem>> {
    let len = unsafe { array.Length()? };
    let mut items = Vec::with_capacity(len.max(0) as usize);
    for index in 0..len {
        let element = unsafe { array.GetElement(index)? };
        if let Ok(item) = item(&element) {
            items.push(item);
        }
    }
    Ok(items)
}

fn item(element: &IUIAutomationElement) -> Result<UiItem> {
    fn text(value: windows::core::Result<BSTR>) -> String {
        value.map(|s| s.to_string()).unwrap_or_default()
    }
    unsafe {
        Ok(UiItem {
            name: text(element.CurrentName()),
            automation_id: text(element.CurrentAutomationId()),
            class_name: text(element.CurrentClassName()),
            control_type: element.CurrentControlType()?.0,
            offscreen: element.CurrentIsOffscreen()?.as_bool(),
            rect: element.CurrentBoundingRectangle()?.into(),
        })
    }
}

pub fn window_rect(hwnd: HWND) -> Result<Rect> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect)
            .ok()
            .context("GetWindowRect failed")?
    };
    Ok(rect.into())
}

pub fn class_name(hwnd: HWND) -> String {
    let mut class = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class) };
    String::from_utf16_lossy(&class[..len as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_geometry() {
        let rect = Rect {
            left: 10,
            top: 20,
            right: 30,
            bottom: 60,
        };
        assert_eq!(rect.width(), 20);
        assert_eq!(rect.height(), 40);
        assert_eq!(rect.center(), (20, 40));
        assert!(!rect.is_empty());
    }
}
