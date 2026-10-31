#![cfg(windows)]

use anyhow::{Context, Result};
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::core::HSTRING;
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItemBuilder, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Clone, Copy)]
pub enum Cmd {
    Edit(&'static str),
    TestTaskbar,
    TestTray,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    items: Vec<(MenuId, Cmd)>,
}

impl Tray {
    pub fn new() -> Result<Self> {
        let icon = TrayIconBuilder::new()
            .with_tooltip("taskbar-tools-rs")
            .with_icon(app_icon()?)
            .build()
            .context("cannot create tray icon")?;
        let mut tray = Self {
            icon,
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

    fn rebuild(&mut self) {
        let menu = Menu::new();
        let mut items = Vec::new();
        for (label, file) in [
            ("Edit taskbar-pin.toml", "taskbar-pin.toml"),
            ("Edit tray-pin.toml", "tray-pin.toml"),
            ("Edit center-taskbar.toml", "center-taskbar.toml"),
            ("Edit start-button.toml", "start-button.toml"),
            ("Edit hidden-taskbar.toml", "hidden-taskbar.toml"),
        ] {
            item(&menu, &mut items, label, Cmd::Edit(file));
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        item(&menu, &mut items, "Test TaskbarPin now", Cmd::TestTaskbar);
        item(&menu, &mut items, "Test TrayPin now", Cmd::TestTray);
        let _ = menu.append(&PredefinedMenuItem::separator());
        item(&menu, &mut items, "Quit", Cmd::Quit);
        self.items = items;
        self.icon.set_menu(Some(Box::new(menu)));
    }
}

fn item(menu: &Menu, items: &mut Vec<(MenuId, Cmd)>, label: &str, cmd: Cmd) {
    let entry = MenuItemBuilder::new().text(label).enabled(true).build();
    items.push((entry.id().clone(), cmd));
    let _ = menu.append(&entry);
}

pub fn edit_config(file: &str) -> Result<()> {
    let path = taskbar_kit::config::exe_dir_file(file)?;
    if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL")) {
        if !editor.is_empty() {
            let editor = editor.to_string_lossy();
            editor_command(&editor, &path)
                .spawn()
                .with_context(|| format!("cannot launch EDITOR={editor}"))?;
            return Ok(());
        }
    }
    let path = HSTRING::from(path.to_string_lossy().as_ref());
    let result = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &path,
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(result.0 as isize > 32, "no handler for {file}");
    Ok(())
}

fn editor_command(editor: &str, path: &std::path::Path) -> Command {
    let mut cmd = Command::new("cmd");
    // /S consumes the outer quotes; the inner quotes protect paths with spaces.
    cmd.args(["/S", "/C"])
        .raw_arg(format!("\"{editor} \"{}\"\"", path.display()))
        .creation_flags(CREATE_NO_WINDOW.0);
    cmd
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
