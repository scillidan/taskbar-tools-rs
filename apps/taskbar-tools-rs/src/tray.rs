#![cfg(windows)]

use anyhow::{Context, Result};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use tray_icon::menu::{
    CheckMenuItemBuilder, Menu, MenuEvent, MenuId, MenuItemBuilder, PredefinedMenuItem,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::core::HSTRING;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const APP_NAME: &str = "taskbar-tools-rs";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Clone, Copy)]
pub enum Cmd {
    ToggleAutostart,
    Reload,
    Edit,
    UnpinAll,
    ToggleSuspend,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    autostart: bool,
    suspended: bool,
    items: Vec<(MenuId, Cmd)>,
}

impl Tray {
    pub fn new() -> Result<Self> {
        let autostart = autostart_enabled();
        let icon = TrayIconBuilder::new()
            .with_tooltip(APP_NAME)
            .with_icon(app_icon()?)
            .build()
            .context("cannot create tray icon")?;
        let mut tray = Self {
            icon,
            autostart,
            suspended: false,
            items: Vec::new(),
        };
        tray.rebuild();
        Ok(tray)
    }

    pub fn poll(&self) -> Option<Cmd> {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(&(_, cmd)) = self.items.iter().find(|(id, _)| *id == event.id) {
                return Some(cmd);
            }
        }
        None
    }

    pub fn toggle_autostart(&mut self) -> Result<()> {
        let on = !self.autostart;
        set_autostart(on)?;
        self.autostart = on;
        self.rebuild();
        Ok(())
    }

    pub fn toggle_hotkeys(&mut self) -> bool {
        self.suspended = !self.suspended;
        self.rebuild();
        self.suspended
    }

    fn rebuild(&mut self) {
        let labels = menu_labels(self.suspended);
        let menu = Menu::new();
        let mut items = Vec::new();
        let auto = CheckMenuItemBuilder::new()
            .text(labels[0])
            .checked(self.autostart)
            .enabled(true)
            .build();
        let _ = menu.append(&auto);
        items.push((auto.id().clone(), Cmd::ToggleAutostart));
        item(&menu, &mut items, labels[1], Cmd::Reload);
        item(&menu, &mut items, labels[2], Cmd::Edit);
        item(&menu, &mut items, labels[3], Cmd::UnpinAll);
        item(&menu, &mut items, labels[4], Cmd::ToggleSuspend);
        let _ = menu.append(&PredefinedMenuItem::separator());
        item(&menu, &mut items, labels[5], Cmd::Quit);
        self.items = items;
        self.icon.set_menu(Some(Box::new(menu)));
    }
}

/// Must match the six-row menu in `desktop-tools-rs`.
fn menu_labels(suspended: bool) -> [&'static str; 6] {
    [
        "Start with Windows",
        "Reload config",
        "Edit config",
        "Unpin all",
        if suspended {
            "Resume hotkeys"
        } else {
            "Suspend hotkeys"
        },
        "Quit",
    ]
}

fn item(menu: &Menu, items: &mut Vec<(MenuId, Cmd)>, label: &str, cmd: Cmd) {
    let entry = MenuItemBuilder::new().text(label).enabled(true).build();
    items.push((entry.id().clone(), cmd));
    let _ = menu.append(&entry);
}

pub fn edit_config() -> Result<()> {
    let path = crate::config::path()?;
    if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL")) {
        if !editor.is_empty() {
            spawn_editor(&editor.to_string_lossy(), &path)?;
            return Ok(());
        }
    }
    let display = path.display().to_string();
    let wide = HSTRING::from(path.to_string_lossy().as_ref());
    let result = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &wide,
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(result.0 as isize > 32, "no handler for {display}");
    Ok(())
}

fn spawn_editor(editor: &str, path: &Path) -> Result<()> {
    editor_command(editor, path)
        .spawn()
        .with_context(|| format!("cannot launch EDITOR={editor}"))?;
    Ok(())
}

fn editor_command(editor: &str, path: &Path) -> Command {
    let mut cmd = Command::new("cmd");
    // /S consumes the outer quotes; the inner quotes protect paths with spaces.
    cmd.args(["/S", "/C"])
        .raw_arg(format!("\"{editor} \"{}\"\"", path.display()))
        .creation_flags(CREATE_NO_WINDOW.0);
    cmd
}

fn run_value(name: &str) -> Option<String> {
    let subkey = HSTRING::from(RUN_KEY);
    let value = HSTRING::from(name);
    let mut len = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey,
            &value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut len),
        )
    };
    if status != ERROR_SUCCESS || len == 0 {
        return None;
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2)];
    let mut bytes = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &subkey,
            &value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    Some(
        String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string(),
    )
}

fn autostart_enabled() -> bool {
    run_value(APP_NAME).is_some()
}

fn set_autostart(on: bool) -> Result<()> {
    let subkey = HSTRING::from(RUN_KEY);
    let value = HSTRING::from(APP_NAME);
    let mut key = HKEY::default();
    let status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_SET_VALUE, &mut key) };
    anyhow::ensure!(
        status == ERROR_SUCCESS,
        "cannot open HKCU\\{RUN_KEY} (error {})",
        status.0
    );
    let result = unsafe {
        if on {
            let exe = std::env::current_exe().context("cannot locate own executable")?;
            let cmd = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(key, &value, Some(0), REG_SZ, Some(bytes))
        } else {
            RegDeleteValueW(key, &value)
        }
    };
    let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
    anyhow::ensure!(
        result == ERROR_SUCCESS,
        "cannot {} the autostart value (error {})",
        if on { "write" } else { "remove" },
        result.0
    );
    Ok(())
}

fn app_icon() -> Result<Icon> {
    const N: usize = 32;
    let mut rgba = vec![0; N * N * 4];
    for y in 5..27 {
        for x in 5..27 {
            let border = x == 5 || x == 26 || y == 5 || y == 26;
            let bars = (8..24).contains(&x) && matches!(y, 10 | 11 | 15 | 16 | 20 | 21);
            if border || bars {
                let offset = (y * N + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&[80, 160, 255, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, N as u32, N as u32).context("invalid tray icon pixels")
}

#[cfg(test)]
mod tests {
    use super::menu_labels;

    #[test]
    fn menu_matches_desktop_tools() {
        assert_eq!(
            menu_labels(false),
            [
                "Start with Windows",
                "Reload config",
                "Edit config",
                "Unpin all",
                "Suspend hotkeys",
                "Quit",
            ]
        );
        assert_eq!(menu_labels(true)[4], "Resume hotkeys");
    }
}
