use super::geometry::{self, Action, ListLayout, Rect};
use anyhow::{Context, Result};
use windows::core::HSTRING;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HWND, LPARAM, RECT as WinRect, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_DWORD, RRF_RT_REG_DWORD,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, TreeScope_Children, TreeScope_Descendants,
    UIA_ClassNamePropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SetWindowPos, HWND_BROADCAST, SMTO_ABORTIFHUNG, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WM_SETTINGCHANGE,
};

const PRIMARY_BAR: &str = "Shell_TrayWnd";
const SECONDARY_BAR: &str = "Shell_SecondaryTrayWnd";
const TASK_LIST: &str = "MSTaskListWClass";
const ADVANCED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const ALIGNMENT_VALUE: &str = "TaskbarAl";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Legacy,
    Native,
}

pub struct Discovery {
    pub bars: Vec<Bar>,
    has_task_lists: bool,
}

impl Discovery {
    pub fn mode(&self) -> Mode {
        if self.has_task_lists {
            Mode::Legacy
        } else {
            Mode::Native
        }
    }
}

pub struct Bar {
    pub class_name: String,
    pub bounds: Rect,
    pub lists: Vec<TaskList>,
}

pub struct TaskList {
    pub hwnd: HWND,
    pub bounds: Rect,
    pub layout: Option<ListLayout>,
}

pub fn with_com<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .context("cannot initialize COM")?;
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _guard = ComGuard;
    f()
}

pub fn discover() -> Result<Discovery> {
    unsafe {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .context("cannot create UI Automation client")?;
        let root = automation.GetRootElement()?;
        let primary = class_condition(&automation, PRIMARY_BAR)?;
        let secondary = class_condition(&automation, SECONDARY_BAR)?;
        let bars_condition = automation.CreateOrCondition(&primary, &secondary)?;
        let task_condition = class_condition(&automation, TASK_LIST)?;
        let bars = root.FindAll(TreeScope_Children, &bars_condition)?;
        let mut result = Vec::new();
        let mut has_task_lists = false;
        for index in 0..bars.Length()? {
            let element = bars.GetElement(index)?;
            let bounds = bounds(&element)?;
            let class_name = element.CurrentClassName()?.to_string();
            let lists = element.FindAll(TreeScope_Descendants, &task_condition)?;
            has_task_lists |= lists.Length()? > 0;
            let mut snapshots = Vec::new();
            for list_index in 0..lists.Length()? {
                let list = lists.GetElement(list_index)?;
                match snapshot_list(&automation, &element, &list) {
                    Ok(snapshot) => snapshots.push(snapshot),
                    Err(error) => eprintln!("center-taskbar: skipping task list: {error:#}"),
                }
            }
            result.push(Bar {
                class_name,
                bounds,
                lists: snapshots,
            });
        }
        Ok(Discovery {
            bars: result,
            has_task_lists,
        })
    }
}

unsafe fn class_condition(
    automation: &IUIAutomation,
    class_name: &str,
) -> Result<windows::Win32::UI::Accessibility::IUIAutomationCondition> {
    Ok(automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &VARIANT::from(class_name))?)
}

unsafe fn snapshot_list(
    automation: &IUIAutomation,
    bar: &IUIAutomationElement,
    list: &IUIAutomationElement,
) -> Result<TaskList> {
    let control = automation.ControlViewWalker()?;
    let raw = automation.RawViewWalker()?;
    let first = optional_bounds(control.GetFirstChildElement(list).ok());
    let last = optional_bounds(control.GetLastChildElement(list).ok());
    let hwnd = list.CurrentNativeWindowHandle()?;
    if hwnd.0.is_null() {
        anyhow::bail!("task list has no native window handle");
    }
    let list_bounds = bounds(list)?;
    let layout = match (first, last) {
        (Some(first), Some(last)) => Some(ListLayout {
            bar: bounds(bar)?,
            list: list_bounds,
            first,
            last,
            previous_sibling: optional_bounds(raw.GetPreviousSiblingElement(list).ok()),
            next_sibling: optional_bounds(raw.GetNextSiblingElement(list).ok()),
            parent: optional_bounds(raw.GetParentElement(list).ok()),
        }),
        _ => None,
    };
    Ok(TaskList {
        hwnd,
        bounds: list_bounds,
        layout,
    })
}

fn optional_bounds(element: Option<IUIAutomationElement>) -> Option<Rect> {
    element.and_then(|element| unsafe { bounds(&element).ok() })
}

unsafe fn bounds(element: &IUIAutomationElement) -> Result<Rect> {
    let WinRect {
        left,
        top,
        right,
        bottom,
    } = element.CurrentBoundingRectangle()?;
    Ok(Rect {
        left,
        top,
        right,
        bottom,
    })
}

pub fn print_discovery(discovery: &Discovery) {
    println!("mode: {:?}", discovery.mode());
    println!("bars: {}", discovery.bars.len());
    for (bar_index, bar) in discovery.bars.iter().enumerate() {
        println!(
            "bar[{bar_index}] class={} bounds={}",
            bar.class_name,
            format_rect(bar.bounds)
        );
        for (list_index, list) in bar.lists.iter().enumerate() {
            println!(
                "  list[{list_index}] hwnd={:p} bounds={}",
                list.hwnd.0,
                format_rect(list.bounds)
            );
            if let Some(layout) = list.layout {
                println!(
                    "    first={} last={}",
                    format_rect(layout.first),
                    format_rect(layout.last)
                );
            }
        }
    }
}

fn format_rect(rect: Rect) -> String {
    format!(
        "[{},{}..{},{}] ({}x{})",
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        rect.width(),
        rect.height()
    )
}

pub fn legacy_step() -> Result<Vec<usize>> {
    let discovery = discover()?;
    let mut lists = Vec::new();
    for list in discovery.bars.iter().flat_map(|bar| &bar.lists) {
        lists.push(list.hwnd.0 as usize);
        if let Err(error) = position_list(list) {
            eprintln!("center-taskbar: position failed: {error:#}");
        }
    }
    Ok(lists)
}

pub fn restore_legacy(lists: &[usize]) {
    for &hwnd in lists {
        if let Err(error) = set_list_position(HWND(hwnd as *mut core::ffi::c_void), 0, 0) {
            eprintln!("center-taskbar: restore failed: {error:#}");
        }
    }
}

fn position_list(list: &TaskList) -> Result<()> {
    let Some(layout) = list.layout.as_ref() else {
        return Ok(());
    };
    let Some(action) = geometry::center(layout) else {
        return Ok(());
    };
    match action {
        Action::Move { x, y } => set_list_position(list.hwnd, x, y),
        Action::Restore => set_list_position(list.hwnd, 0, 0),
        Action::Unchanged => Ok(()),
    }
}

fn set_list_position(hwnd: HWND, x: i32, y: i32) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
    }
    .context("SetWindowPos failed")
}

pub struct NativeAlignment {
    previous: Option<u32>,
    changed: bool,
}

impl NativeAlignment {
    pub fn enable() -> Result<Self> {
        let previous = read_alignment()?;
        if previous == Some(1) {
            return Ok(Self {
                previous,
                changed: false,
            });
        }
        write_alignment(Some(1)).context("cannot enable native centered taskbar alignment")?;
        broadcast_settings_change();
        Ok(Self {
            previous,
            changed: true,
        })
    }

    pub fn restore(&mut self) -> Result<()> {
        if !self.changed {
            return Ok(());
        }
        write_alignment(self.previous).context("cannot restore native taskbar alignment")?;
        self.changed = false;
        broadcast_settings_change();
        Ok(())
    }
}

impl Drop for NativeAlignment {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("center-taskbar: {error:#}");
        }
    }
}

fn read_alignment() -> Result<Option<u32>> {
    let subkey = HSTRING::from(ADVANCED_KEY);
    let name = HSTRING::from(ALIGNMENT_VALUE);
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey,
            &name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut _ as *mut _),
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    status.ok().context("cannot read TaskbarAl")?;
    Ok(Some(value))
}

fn write_alignment(value: Option<u32>) -> Result<()> {
    let subkey = HSTRING::from(ADVANCED_KEY);
    let name = HSTRING::from(ALIGNMENT_VALUE);
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_SET_VALUE, &mut key)
            .ok()
            .context("cannot open Explorer Advanced registry key")?;
        let result = match value {
            Some(value) => RegSetValueExW(
                key,
                &name,
                Some(0),
                REG_DWORD,
                Some(std::slice::from_raw_parts(
                    &value as *const _ as *const u8,
                    std::mem::size_of::<u32>(),
                )),
            )
            .ok(),
            None => windows::Win32::System::Registry::RegDeleteValueW(key, &name).ok(),
        };
        let _ = RegCloseKey(key);
        Ok(result?)
    }
}

fn broadcast_settings_change() {
    let parameter = HSTRING::from(ALIGNMENT_VALUE);
    let mut result = 0usize;
    let status = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(parameter.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        )
    };
    if status.0 == 0 {
        eprintln!("center-taskbar: WM_SETTINGCHANGE broadcast failed");
    }
}
