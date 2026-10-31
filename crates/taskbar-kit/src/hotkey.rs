//! Global hotkeys via `RegisterHotKey`, dispatched through the tray message loop.
//!
//! `RegisterHotKey` is enough for plain trigger hotkeys (no key swallowing).
//! Apps that must swallow keys (keyboard-locker) need a `WH_KEYBOARD_LL` hook
//! instead — port from keymap-rs when implementing that app.

use crate::keys::{Combo, Key};
use anyhow::{Context, Result};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    VIRTUAL_KEY,
};

/// Map a combo to a virtual key (for RegisterHotKey).
pub fn vk_of_key(key: Key) -> Option<VIRTUAL_KEY> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    Some(match key {
        Key::Char(c) => VIRTUAL_KEY(c.to_ascii_uppercase() as u16),
        Key::F(n) => VIRTUAL_KEY(VK_F1.0 + (n as u16) - 1),
        Key::NumpadDot => VK_DECIMAL,
        Key::Insert => VK_INSERT,
        Key::Delete => VK_DELETE,
        Key::Space => VK_SPACE,
        Key::Tab => VK_TAB,
        Key::CapsLock => VK_CAPITAL,
        Key::PrintScreen => VK_PRINT,
        Key::Backspace => VK_BACK,
        Key::Escape => VK_ESCAPE,
        Key::Left => VK_LEFT,
        Key::Right => VK_RIGHT,
        Key::Up => VK_UP,
        Key::Down => VK_DOWN,
        Key::Home => VK_HOME,
        Key::End => VK_END,
        Key::PageUp => VK_PRIOR,
        Key::PageDown => VK_NEXT,
        Key::Other => return None,
    })
}

/// A registered set of label -> combo hotkeys. Ids run from 1, matching the
/// `id -> label` slice handed to [`crate::message::run`].
pub struct Hotkeys {
    labels: Vec<String>,
}

impl Hotkeys {
    /// Register all combos. Fails if any registration fails (e.g. the combo
    /// is already owned by another app).
    pub fn register(entries: &[(&str, Combo)]) -> Result<Hotkeys> {
        let mut labels = Vec::new();
        for (i, (label, combo)) in entries.iter().enumerate() {
            let id = (i + 1) as i32;
            let mut mods = MOD_NOREPEAT;
            if combo.ctrl {
                mods |= MOD_CONTROL;
            }
            if combo.shift {
                mods |= MOD_SHIFT;
            }
            if combo.alt {
                mods |= MOD_ALT;
            }
            if combo.win {
                mods |= MOD_WIN;
            }
            let vk = vk_of_key(combo.key)
                .with_context(|| format!("hotkey `{combo}` has no virtual key"))?;
            unsafe { RegisterHotKey(None, id, mods, vk.0 as u32) }
                .with_context(|| format!("cannot register hotkey `{combo}` (id {id})"))?;
            labels.push(label.to_string());
        }
        Ok(Hotkeys { labels })
    }

    pub fn label_of(&self, id: i32) -> Option<&str> {
        self.labels.get((id - 1) as usize).map(String::as_str)
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        for (i, _) in self.labels.iter().enumerate() {
            unsafe {
                let _ = UnregisterHotKey(None, (i + 1) as i32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    #[test]
    fn vk_mapping() {
        assert_eq!(vk_of_key(Key::Char('c')).unwrap().0, VK_C.0);
        assert_eq!(vk_of_key(Key::Char('C')).unwrap().0, VK_C.0);
        assert_eq!(vk_of_key(Key::F(1)).unwrap().0, VK_F1.0);
        assert_eq!(vk_of_key(Key::F(12)).unwrap().0, VK_F12.0);
        assert_eq!(vk_of_key(Key::Insert).unwrap().0, VK_INSERT.0);
        assert_eq!(vk_of_key(Key::Other), None);
    }
}
