//! Win32 helpers shared by all apps: single instance, HKCU Run autostart,
//! message boxes, opening folders. Ported from keymap-rs `platform/windows.rs`.

use anyhow::Result;
use std::path::Path;
use windows::Win32::Foundation::HANDLE;

pub const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Named mutex; a second instance exits while this guard is alive.
pub struct SingleInstanceGuard {
    _handle: HANDLE,
}

/// Bail with a message box if another instance is already running.
pub fn ensure_single_instance(name: &str) -> Result<SingleInstanceGuard> {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    let mutex_name = windows::core::HSTRING::from(format!("Local\\taskbar-tools-rs-{name}"));
    let handle = unsafe { CreateMutexW(None, true, &mutex_name)? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        message_box(name, &format!("{name} is already running."));
        anyhow::bail!("{name} is already running");
    }
    Ok(SingleInstanceGuard { _handle: handle })
}

/// Is `value_name` registered in HKCU Run?
pub fn autostart_enabled(value_name: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let subkey = HSTRING::from(RUN_KEY_PATH);
    let value = HSTRING::from(value_name);
    let mut len: u32 = 0;
    let r = unsafe {
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
    r.is_ok() && len > 0
}

/// Register/remove the current exe in HKCU Run under `value_name`.
pub fn set_autostart(value_name: &str, on: bool) -> Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };
    let subkey = HSTRING::from(RUN_KEY_PATH);
    let value = HSTRING::from(value_name);
    let mut key = windows::Win32::System::Registry::HKEY::default();
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, Some(0), KEY_SET_VALUE, &mut key).ok()?;
        if on {
            let exe = format!("\"{}\"", std::env::current_exe()?.display());
            let wide: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
            RegSetValueExW(
                key,
                &value,
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide.as_ptr() as *const u8,
                    wide.len() * 2,
                )),
            )
            .ok()?;
        } else {
            let _ = RegDeleteValueW(key, &value);
        }
        let _ = RegCloseKey(key);
    }
    Ok(())
}

/// Show a modal error dialog.
pub fn message_box(title: &str, text: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let title = HSTRING::from(title);
    let text = HSTRING::from(text);
    unsafe {
        MessageBoxW(None, &text, &title, MB_OK | MB_ICONERROR);
    }
}

/// Yes/No question; true when the user picked Yes.
pub fn message_box_yesno(title: &str, text: &str) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONQUESTION, MB_YESNO};
    let title = HSTRING::from(title);
    let text = HSTRING::from(text);
    unsafe { MessageBoxW(None, &text, &title, MB_YESNO | MB_ICONQUESTION) == IDYES }
}

/// Hide a console created only for a double-clicked daemon, never a parent's terminal.
pub fn hide_owned_console() {
    use windows::Win32::System::Console::{GetConsoleProcessList, GetConsoleWindow};
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let console = GetConsoleWindow();
        let mut processes = [0u32; 1];
        if !console.0.is_null() && GetConsoleProcessList(&mut processes) == 1 {
            let _ = ShowWindow(console, SW_HIDE);
        }
    }
}

/// Attach CLI diagnostics to a parent terminal if the process has no console.
pub fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

pub fn post_thread_message(thread: u32, wparam: usize, lparam: isize) -> Result<()> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};
    unsafe { PostThreadMessageW(thread, WM_APP, WPARAM(wparam), LPARAM(lparam))? };
    Ok(())
}

/// Append a diagnostic for GUI-subsystem apps, next to the executable.
pub fn log(app: &str, text: &str) {
    use std::io::Write;
    if let Ok(path) = crate::config::exe_dir_file(&format!("{app}.log")) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{text}");
        }
    }
}

/// Open a folder in Explorer.
pub fn open_folder(path: &Path) {
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}

/// Open a file or URL with its default handler.
pub fn open_with_default(path: &Path) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let file = HSTRING::from(path.to_string_lossy().as_ref());
    unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &file,
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

/// Run a command line with ShellExecute (like AHK `Run`).
pub fn run_command(cmd: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let file = HSTRING::from(cmd);
    unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &file,
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

// ------------------------------------------------------- window / process

use windows::Win32::Foundation::HWND;

/// The foreground window, if any.
pub fn foreground_hwnd() -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.0.is_null()).then_some(hwnd)
}

/// The window under the cursor (`WindowFromPoint`, like AHK `MouseGetPos`).
pub fn hwnd_under_cursor() -> Option<HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, WindowFromPoint};
    let mut pt = windows::Win32::Foundation::POINT::default();
    let got = unsafe { GetCursorPos(&mut pt) };
    if got.is_err() {
        return None;
    }
    let hwnd = unsafe { WindowFromPoint(pt) };
    (!hwnd.0.is_null()).then_some(hwnd)
}

/// Process file name (e.g. `notepad.exe`) of the window's owner, lowercased.
pub fn process_name(hwnd: HWND) -> Option<String> {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
    };
    if !ok {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
}

// ----------------------------------------------------------------- input

fn key_input(
    vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[windows::Win32::UI::Input::KeyboardAndMouse::INPUT]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;
    unsafe {
        SendInput(
            inputs,
            std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
    }
}

/// Press and release a virtual key (like AHK `Send {Blind}{Esc}`).
pub fn send_key(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP};
    send(&[
        key_input(vk, KEYBD_EVENT_FLAGS(0)),
        key_input(vk, KEYEVENTF_KEYUP),
    ]);
}

/// Inject Ctrl+C (AHK `Send ^c`).
pub fn send_ctrl_c() {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    send(&[
        key_input(VK_LCONTROL, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_C, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_C, KEYEVENTF_KEYUP),
        key_input(VK_LCONTROL, KEYEVENTF_KEYUP),
    ]);
}

// ------------------------------------------------------------------ tips

/// Show `text` in a tracking tooltip centered on the active window (or the
/// screen), for `duration` — the AHK `ToolTip` pattern used by several tools.
pub fn center_tip(text: &str, duration: std::time::Duration) {
    let text = text.to_string();
    std::thread::spawn(move || {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::Controls::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let Ok(hwnd) = CreateWindowExW(
                WS_EX_TOPMOST,
                windows::core::w!("tooltips_class32"),
                None,
                WS_POPUP | WINDOW_STYLE(TTS_NOPREFIX | TTS_ALWAYSTIP),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                None,
                None,
            ) else {
                return;
            };
            let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let ti = TTTOOLINFOW {
                cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                uFlags: TTF_TRACK | TTF_ABSOLUTE,
                hwnd: HWND(std::ptr::null_mut()),
                uId: 0,
                rect: Default::default(),
                hinst: windows::Win32::Foundation::HINSTANCE(std::ptr::null_mut()),
                lpszText: windows::core::PWSTR(wide.as_mut_ptr()),
                lParam: LPARAM(0),
                lpReserved: std::ptr::null_mut(),
            };
            SendMessageW(
                hwnd,
                TTM_ADDTOOLW,
                None,
                Some(LPARAM(&ti as *const _ as isize)),
            );
            let Some((x, y)) = foreground_center() else {
                let _ = DestroyWindow(hwnd);
                return;
            };
            let pos = ((x & 0xffff) | ((y & 0xffff) << 16)) as isize;
            SendMessageW(hwnd, TTM_TRACKPOSITION, None, Some(LPARAM(pos)));
            SendMessageW(
                hwnd,
                TTM_TRACKACTIVATE,
                Some(WPARAM(1)),
                Some(LPARAM(&ti as *const _ as isize)),
            );
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            std::thread::sleep(duration);
            let _ = DestroyWindow(hwnd);
        }
    });

    /// Center point of the foreground window, or the screen.
    #[cfg(windows)]
    fn foreground_center() -> Option<(i32, i32)> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, GetWindowRect, SM_CXSCREEN, SM_CYSCREEN,
        };
        unsafe {
            if let Some(hwnd) = foreground_hwnd() {
                let mut rc = windows::Win32::Foundation::RECT::default();
                if GetWindowRect(hwnd, &mut rc).is_ok() {
                    return Some(((rc.left + rc.right) / 2, (rc.top + rc.bottom) / 2));
                }
            }
            Some((
                GetSystemMetrics(SM_CXSCREEN) / 2,
                GetSystemMetrics(SM_CYSCREEN) / 2,
            ))
        }
    }
}

// ------------------------------------------------------- keyboard layout

/// Display name of a keyboard layout: `HKLM\...\Keyboard Layouts\<klid>`
/// value `Layout Text` (e.g. klid 0x409 -> "US"). None if not found.
pub fn layout_text(klid_loword: u32) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let subkey = HSTRING::from(format!(
        r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts\{klid_loword:08X}"
    ));
    let value = HSTRING::from("Layout Text");
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &subkey,
            &value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut len),
        )
    };
    r.ok()
        .ok()
        .filter(|_| len >= 2)
        .map(|_| String::from_utf16_lossy(&buf[..(len as usize / 2 - 1)]))
        .filter(|s| !s.is_empty())
}

/// HKL of the keyboard layout attached to the window's input thread.
pub fn layout_of_window(hwnd: HWND) -> Option<isize> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    let mut pid = 0u32;
    let tid = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (tid != 0).then(|| unsafe { GetKeyboardLayout(tid).0 as isize })
}
