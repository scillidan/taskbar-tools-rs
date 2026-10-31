//! Explorer item discovery and the final, isolated physical drag operation.

use crate::pin::{moves_to, Item};
use crate::shell::{taskbars, Automation, Rect, UiItem};
use anyhow::{Context, Result};
use std::collections::HashSet;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Accessibility::UIA_MenuItemControlTypeId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MOVE, MOUSEINPUT, MOUSE_EVENT_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

/// All modules share the physical mouse, so their drag sequences must not overlap.
pub static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug)]
pub struct Group {
    pub key: String,
    pub items: Vec<UiItem>,
    pub vertical: bool,
    pub grid: bool,
}

impl Group {
    pub fn ids(&self) -> Vec<String> {
        self.items.iter().map(UiItem::identity).collect()
    }

    pub fn pin_items(&self) -> Vec<Item> {
        self.items
            .iter()
            .map(|item| Item::new(item.identity(), item.name.clone()))
            .collect()
    }
}

pub fn taskbar_groups(automation: &Automation) -> Result<Vec<Group>> {
    let mut groups = Vec::new();
    for bar in taskbars()? {
        let mut items = taskbar_items(automation.descendants_of(bar.hwnd)?);
        if bar.rect.width() >= bar.rect.height() {
            items.sort_by_key(|item| item.rect.left);
        }
        groups.push(Group {
            key: format!("taskbar-{:x}", bar.hwnd.0 as usize),
            items,
            vertical: bar.rect.height() > bar.rect.width(),
            grid: false,
        });
    }
    Ok(groups)
}

fn taskbar_items(items: Vec<UiItem>) -> Vec<UiItem> {
    let lists = container_rects(&items, "MSTaskListWClass");
    let mut items: Vec<UiItem> = items
        .into_iter()
        .filter(|item| {
            !item.offscreen
                && !item.rect.is_empty()
                && ((item.is_button()
                    && (item.class_name == "Taskbar.TaskListButtonAutomationPeer"
                        || item.automation_id.starts_with("Appid:")))
                    || ((item.is_button() || item.control_type == UIA_MenuItemControlTypeId.0)
                        && within_any(item.rect, &lists)))
        })
        .collect();
    items.sort_by_key(|item| (item.rect.top, item.rect.left));
    items
}

pub fn tray_groups(automation: &Automation) -> Result<Vec<Group>> {
    let mut groups = Vec::new();
    for bar in taskbars()? {
        let items = tray_items(automation.descendants_of(bar.hwnd)?);
        groups.push(Group {
            key: format!("visible-{:x}", bar.hwnd.0 as usize),
            items,
            vertical: bar.rect.height() > bar.rect.width(),
            grid: true,
        });
    }
    Ok(groups)
}

fn tray_items(items: Vec<UiItem>) -> Vec<UiItem> {
    let toolbars = container_rects(&items, "ToolbarWindow32");
    let mut seen = HashSet::new();
    let mut items: Vec<UiItem> = items
        .into_iter()
        .filter(|item| {
            !item.offscreen
                && !item.rect.is_empty()
                && item.is_button()
                && (item.automation_id == "NotifyItemIcon" || within_any(item.rect, &toolbars))
        })
        .filter(|item| seen.insert(item.identity()))
        .collect();
    items.sort_by_key(|item| (item.rect.top, item.rect.left));
    items
}

fn container_rects(items: &[UiItem], class_name: &str) -> Vec<Rect> {
    items
        .iter()
        .filter(|item| item.class_name == class_name && !item.rect.is_empty())
        .map(|item| item.rect)
        .collect()
}

fn within_any(rect: Rect, containers: &[Rect]) -> bool {
    let (x, y) = rect.center();
    containers.iter().any(|container| {
        x >= container.left && x < container.right && y >= container.top && y < container.bottom
    })
}

pub fn drag_move(group: &Group, id: &str, target_index: usize) -> Result<()> {
    let _drag = LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let source_index = group
        .items
        .iter()
        .position(|item| item.identity() == id)
        .context("drag source disappeared")?;
    let source = group.items[source_index].rect;
    let target = insertion_point(group, source_index, target_index)?;
    drag(source, target)
}

fn insertion_point(group: &Group, source_index: usize, target_index: usize) -> Result<(i32, i32)> {
    if target_index < group.items.len() {
        let target = group.items[target_index].rect;
        let mut point = target.center();
        if group.grid {
            // The upper/left half selects insertion before the target in a grid.
            if source_index < target_index {
                point.0 = target.left + target.width() / 4;
            } else {
                point.0 = target.right - target.width() / 4;
            }
        } else if group.vertical {
            point.1 = if source_index < target_index {
                target.bottom - target.height() / 4
            } else {
                target.top + target.height() / 4
            };
        } else {
            point.0 = if source_index < target_index {
                target.right - target.width() / 4
            } else {
                target.left + target.width() / 4
            };
        }
        return Ok(point);
    }
    let last = group
        .items
        .last()
        .context("cannot sort an empty group")?
        .rect;
    Ok(if group.grid {
        (last.right + last.width() / 2, last.center().1)
    } else if group.vertical {
        (last.center().0, last.bottom + last.height() / 2)
    } else {
        (last.right + last.width() / 2, last.center().1)
    })
}

fn drag(source: Rect, target: (i32, i32)) -> Result<()> {
    unsafe {
        let mut old = POINT::default();
        GetCursorPos(&mut old).ok().context("GetCursorPos failed")?;
        let from = source.center();
        SetCursorPos(from.0, from.1)
            .ok()
            .context("SetCursorPos failed")?;
        let mut pressed = false;
        let result = (|| -> Result<()> {
            mouse(MOUSEEVENTF_LEFTDOWN)?;
            pressed = true;
            std::thread::sleep(std::time::Duration::from_millis(80));
            for step in 1..=8 {
                let x = from.0 + (target.0 - from.0) * step / 8;
                let y = from.1 + (target.1 - from.1) * step / 8;
                SetCursorPos(x, y).ok().context("SetCursorPos failed")?;
                mouse(MOUSEEVENTF_MOVE)?;
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
            Ok(())
        })();
        let released = if pressed {
            mouse(MOUSEEVENTF_LEFTUP)
        } else {
            Ok(())
        };
        let restored = SetCursorPos(old.x, old.y)
            .ok()
            .context("cannot restore cursor");
        result?;
        released?;
        restored?;
    }
    Ok(())
}

fn mouse(flags: MOUSE_EVENT_FLAGS) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        anyhow::bail!("SendInput did not send the mouse event");
    }
    Ok(())
}

pub fn first_move(current: &[String], target: &[String]) -> Option<(String, usize)> {
    moves_to(current, target).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_move_only() {
        let current = vec!["a".into(), "b".into(), "c".into()];
        let target = vec!["c".into(), "b".into(), "a".into()];
        assert_eq!(first_move(&current, &target), Some(("c".into(), 0)));
    }

    fn item(name: &str, id: &str, class: &str, control_type: i32, rect: Rect) -> UiItem {
        UiItem {
            name: name.into(),
            automation_id: id.into(),
            class_name: class.into(),
            control_type,
            offscreen: false,
            rect,
        }
    }

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn legacy_taskbar_buttons_are_scoped_to_the_task_list() {
        let button = windows::Win32::UI::Accessibility::UIA_ButtonControlTypeId.0;
        let items = vec![
            item("apps", "", "MSTaskListWClass", 50021, rect(0, 30, 62, 1200)),
            item("Brave", "Brave", "", button, rect(0, 33, 62, 64)),
            item(
                "WezTerm",
                "org.wezfurlong.wezterm",
                "",
                UIA_MenuItemControlTypeId.0,
                rect(0, 64, 62, 95),
            ),
            item("Network", "network", "", button, rect(0, 1250, 62, 1280)),
        ];
        let ids: Vec<String> = taskbar_items(items).iter().map(UiItem::identity).collect();
        assert_eq!(ids, ["Brave|Brave", "org.wezfurlong.wezterm|WezTerm"]);
    }

    #[test]
    fn legacy_tray_buttons_are_scoped_to_toolbars() {
        let button = windows::Win32::UI::Accessibility::UIA_ButtonControlTypeId.0;
        let items = vec![
            item(
                "tray",
                "1504",
                "ToolbarWindow32",
                50021,
                rect(0, 1200, 62, 1280),
            ),
            item("Steam", "", "", button, rect(0, 1210, 24, 1234)),
            item("Brave", "Brave", "", button, rect(0, 33, 62, 64)),
        ];
        let ids: Vec<String> = tray_items(items).iter().map(UiItem::identity).collect();
        assert_eq!(ids, ["Steam"]);
    }
}
