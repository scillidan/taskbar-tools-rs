#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    fn width(self) -> i32 {
        self.right - self.left
    }

    fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dock {
    Left,
    Top,
    Right,
    Bottom,
}

fn dock(taskbar: Rect, monitor: Rect) -> Dock {
    if taskbar.width() > taskbar.height() {
        if taskbar.top + taskbar.bottom <= monitor.top + monitor.bottom {
            Dock::Top
        } else {
            Dock::Bottom
        }
    } else if taskbar.left + taskbar.right <= monitor.left + monitor.right {
        Dock::Left
    } else {
        Dock::Right
    }
}

/// Extend only the window edge exactly touching the taskbar's inner edge.
pub fn extend_snapped(window: Rect, taskbar: Rect, monitor: Rect) -> Option<Rect> {
    let mut extended = window;
    match dock(taskbar, monitor) {
        Dock::Left if window.left == taskbar.right => extended.left = taskbar.left,
        Dock::Top if window.top == taskbar.bottom => extended.top = taskbar.top,
        Dock::Right if window.right == taskbar.left => extended.right = taskbar.right,
        Dock::Bottom if window.bottom == taskbar.top => extended.bottom = taskbar.bottom,
        _ => return None,
    }
    Some(extended)
}

pub fn selectable(
    visible: bool,
    minimized: bool,
    maximized: bool,
    tool: bool,
    class: &str,
) -> bool {
    visible
        && !minimized
        && !maximized
        && !tool
        && !matches!(
            class,
            "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        left: -1920,
        top: 0,
        right: 0,
        bottom: 1080,
    };

    fn base_window() -> Rect {
        Rect {
            left: -1920,
            top: 0,
            right: -960,
            bottom: 1032,
        }
    }

    #[test]
    fn extends_bottom_snapped_window() {
        let taskbar = Rect {
            left: -1920,
            top: 1032,
            right: 0,
            bottom: 1080,
        };
        let extended = extend_snapped(base_window(), taskbar, MONITOR).unwrap();
        assert_eq!(extended.bottom, 1080);
        assert_eq!(extended.left, base_window().left);
    }

    #[test]
    fn extends_each_docked_edge() {
        let taskbar = Rect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 48,
        };
        let window = Rect {
            top: 48,
            ..base_window()
        };
        assert_eq!(extend_snapped(window, taskbar, MONITOR).unwrap().top, 0);

        let taskbar = Rect {
            left: -1920,
            top: 0,
            right: -1840,
            bottom: 1080,
        };
        let window = Rect {
            left: -1840,
            ..base_window()
        };
        assert_eq!(
            extend_snapped(window, taskbar, MONITOR).unwrap().left,
            -1920
        );

        let taskbar = Rect {
            left: -80,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        let window = Rect {
            right: -80,
            ..base_window()
        };
        assert_eq!(extend_snapped(window, taskbar, MONITOR).unwrap().right, 0);
    }

    #[test]
    fn leaves_unsnapped_window_unchanged() {
        let taskbar = Rect {
            left: -1920,
            top: 1032,
            right: 0,
            bottom: 1080,
        };
        assert_eq!(
            extend_snapped(
                Rect {
                    bottom: 1000,
                    ..base_window()
                },
                taskbar,
                MONITOR
            ),
            None
        );
    }

    #[test]
    fn selects_only_normal_visible_non_shell_windows() {
        assert!(selectable(true, false, false, false, "Notepad"));
        assert!(!selectable(false, false, false, false, "Notepad"));
        assert!(!selectable(true, true, false, false, "Notepad"));
        assert!(!selectable(true, false, true, false, "Notepad"));
        assert!(!selectable(true, false, false, true, "Notepad"));
        for class in [
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "Progman",
            "WorkerW",
        ] {
            assert!(!selectable(true, false, false, false, class));
        }
    }
}
