//! Win32 message loop for timers, wakeups and registered hotkeys.

use crate::hotkey::Hotkeys;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, WM_APP, WM_HOTKEY, WM_TIMER,
};

pub enum Event {
    Hotkey(String),
    Wakeup(usize),
    Timer(usize),
}

pub fn run(hotkeys: Option<&Hotkeys>, mut on_event: impl FnMut(Event)) {
    unsafe {
        let mut msg = MSG::default();
        loop {
            if GetMessageW(&mut msg, None, 0, 0).0 <= 0 {
                break;
            }
            DispatchMessageW(&msg);
            match msg.message {
                WM_APP => on_event(Event::Wakeup(msg.wParam.0)),
                WM_TIMER => on_event(Event::Timer(msg.wParam.0)),
                WM_HOTKEY => {
                    if let Some(label) = hotkeys.and_then(|keys| keys.label_of(msg.wParam.0 as i32))
                    {
                        on_event(Event::Hotkey(label.to_string()));
                    }
                }
                _ => {}
            }
        }
    }
}
