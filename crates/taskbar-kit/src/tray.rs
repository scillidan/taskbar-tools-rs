//! Tray icon + menu + Win32 message loop, generic over all apps.
//!
//! Menu convention (from the AHK projects): a `>` prefix marks the active
//! state of a toggle item.
//!
//! One loop dispatches both menu clicks and [`Hotkeys`](crate::hotkey::Hotkeys)
//! events to a single app closure:

use crate::hotkey::Hotkeys;
use anyhow::{Context, Result};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, MenuItemBuilder, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostQuitMessage, MSG,
};

/// One menu row.
pub enum Entry {
    Sep,
    /// Clickable item with a stable label.
    Item(&'static str),
    /// Toggle item; rendered with a `>` prefix when on.
    Toggle(&'static str, bool),
}

/// An event from the message loop.
pub enum Event {
    /// A menu item was clicked (`Item` or `Toggle` label).
    Menu(String),
    /// A registered hotkey fired (its label).
    Hotkey(String),
    /// `WM_APP` posted to the loop's thread (state changed elsewhere, e.g. in a
    /// hook or timer thread). The `usize` is the message's `WPARAM.0` — apps
    /// use it to tell wakeup reasons apart.
    Wakeup(usize),
    /// Thread timer (`SetTimer(None, id, ...)`) fired.
    Timer(usize),
}

pub struct Tray {
    icon: TrayIcon,
    tooltip: String,
    ids: Vec<(MenuId, String)>,
}

impl Tray {
    pub fn new(tooltip: &str, rgba: Vec<u8>) -> Result<Tray> {
        let icon = Icon::from_rgba(rgba, 32, 32).context("icon error")?;
        let (menu, ids) = build_menu(&[]);
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()
            .context("tray icon")?;
        Ok(Tray {
            icon,
            tooltip: tooltip.to_string(),
            ids,
        })
    }

    /// Re-render the menu. Call after app state changes (toggle flips).
    pub fn set_menu(&mut self, entries: &[Entry]) {
        let (menu, ids) = build_menu(entries);
        self.ids = ids;
        self.icon.set_menu(Some(Box::new(menu)));
    }

    pub fn set_tooltip(&mut self, text: &str) {
        self.tooltip = text.to_string();
        let _ = self.icon.set_tooltip(Some(&self.tooltip));
    }

    /// Swap the tray icon (e.g. locked/unlocked states).
    pub fn set_icon(&mut self, rgba: Vec<u8>) -> Result<()> {
        let icon = Icon::from_rgba(rgba, 32, 32).context("icon error")?;
        let _ = self.icon.set_icon(Some(icon));
        Ok(())
    }

    /// Stop [`run`](Tray::run).
    pub fn quit(&self) {
        unsafe { PostQuitMessage(0) };
    }

    /// Message loop until quit. `on_event` receives menu, hotkey, and wakeup
    /// events. [`Event::Wakeup`] is fired when a `WM_APP` message is posted to
    /// this thread (e.g. from a hook proc that changed app state).
    pub fn run(&mut self, hotkeys: Option<&Hotkeys>, mut on_event: impl FnMut(&mut Tray, Event)) {
        unsafe {
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, None, 0, 0);
                if ret.0 <= 0 {
                    break;
                }
                DispatchMessageW(&msg);
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_APP {
                    on_event(self, Event::Wakeup(msg.wParam.0));
                }
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_TIMER {
                    on_event(self, Event::Timer(msg.wParam.0));
                }
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY {
                    if let Some(hotkeys) = hotkeys {
                        if let Some(label) = hotkeys.label_of(msg.wParam.0 as i32) {
                            on_event(self, Event::Hotkey(label.to_string()));
                        }
                    }
                }
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    if let Some(label) = self.label_of(&event.id) {
                        on_event(self, Event::Menu(label));
                    }
                }
            }
        }
    }

    fn label_of(&self, id: &MenuId) -> Option<String> {
        self.ids
            .iter()
            .find(|(mid, _)| mid == id)
            .map(|(_, label)| label.clone())
    }
}

fn build_menu(entries: &[Entry]) -> (Menu, Vec<(MenuId, String)>) {
    let menu = Menu::new();
    let mut ids = Vec::new();
    for entry in entries {
        match entry {
            Entry::Sep => {
                let _ = menu.append(&PredefinedMenuItem::separator());
            }
            Entry::Item(label) => {
                // muda's builder defaults to disabled, which greys out every row.
                let item = MenuItemBuilder::new().text(*label).enabled(true).build();
                ids.push((item.id().clone(), label.to_string()));
                let _ = menu.append(&item);
            }
            Entry::Toggle(label, on) => {
                let text = if *on {
                    format!("> {label}")
                } else {
                    label.to_string()
                };
                let item: MenuItem = MenuItemBuilder::new().text(text).enabled(true).build();
                ids.push((item.id().clone(), label.to_string()));
                let _ = menu.append(&item);
            }
        }
    }
    (menu, ids)
}
