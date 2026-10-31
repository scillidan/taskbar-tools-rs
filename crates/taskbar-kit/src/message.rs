//! Win32 message loop for timers, wakeups and registered hotkeys.

use crate::hotkey::Hotkeys;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, WM_APP, WM_HOTKEY, WM_TIMER,
};

pub enum Event {
    Hotkey(String),
    Wakeup(usize),
    Timer(usize),
    /// A dispatched window message; useful for polling cross-thread event channels.
    Message,
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
                _ => on_event(Event::Message),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        PeekMessageW, PostQuitMessage, PostThreadMessageW, PM_NOREMOVE, WM_USER,
    };

    #[test]
    fn plain_messages_dispatch_without_a_timer() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            unsafe {
                let mut message = MSG::default();
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
            }
            sender.send(unsafe { GetCurrentThreadId() }).unwrap();
            let mut seen = false;
            run(None, |event| {
                if matches!(event, Event::Message) {
                    seen = true;
                    unsafe { PostQuitMessage(0) };
                }
            });
            seen
        });
        let thread_id = receiver.recv().unwrap();
        unsafe {
            PostThreadMessageW(thread_id, WM_USER, WPARAM(0), LPARAM(0)).unwrap();
        }
        assert!(worker.join().unwrap());
    }
}
