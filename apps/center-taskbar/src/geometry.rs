#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    pub fn height(self) -> i32 {
        self.bottom - self.top
    }

    fn is_empty(self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ListLayout {
    pub bar: Rect,
    pub list: Rect,
    pub first: Rect,
    /// The last control child's start is the content end; Explorer's final
    /// empty child deliberately has a nonzero extent.
    pub last: Rect,
    pub previous_sibling: Option<Rect>,
    pub next_sibling: Option<Rect>,
    pub parent: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Move { x: i32, y: i32 },
    Restore,
    Unchanged,
}

/// Calculate a child-window position from physical-pixel UIA rectangles.
pub fn center(layout: &ListLayout) -> Option<Action> {
    let horizontal = layout.bar.width() > layout.bar.height();
    let (bar_start, bar_extent, list_start, first_start, last_start) = if horizontal {
        (
            layout.bar.left,
            layout.bar.width(),
            layout.list.left,
            layout.first.left,
            layout.last.left,
        )
    } else {
        (
            layout.bar.top,
            layout.bar.height(),
            layout.list.top,
            layout.first.top,
            layout.last.top,
        )
    };
    let content = i64::from(last_start) - i64::from(first_start);
    if content <= 0 || bar_extent <= 0 {
        return None;
    }

    // The first-child offset is stable when the whole list window moves.
    let padding = (i64::from(first_start) - i64::from(list_start)).max(0);
    let left = layout
        .previous_sibling
        .filter(|r| !r.is_empty())
        .map(|r| i64::from(if horizontal { r.right } else { r.bottom }))
        .or_else(|| {
            layout
                .parent
                .map(|r| i64::from(if horizontal { r.left } else { r.top }) + padding)
        })
        .unwrap_or_else(|| i64::from(bar_start) + padding);
    let right = layout
        .next_sibling
        .filter(|r| !r.is_empty())
        .map(|r| i64::from(if horizontal { r.left } else { r.top }))
        .or_else(|| {
            layout
                .parent
                .map(|r| i64::from(if horizontal { r.right } else { r.bottom }))
        })
        .unwrap_or_else(|| i64::from(bar_start) + i64::from(bar_extent));

    let mut target = (i64::from(bar_start) as f64 + (i64::from(bar_extent) - content) as f64 / 2.0)
        .round() as i64;
    if target + content > right {
        target = right - content;
    }

    if target <= left {
        let restored = left - padding;
        return Some(if (i64::from(list_start) - restored).abs() <= 1 {
            Action::Unchanged
        } else {
            Action::Restore
        });
    }
    if (target - i64::from(first_start)).abs() <= 1 {
        return Some(Action::Unchanged);
    }

    let relative = (target - left).clamp(0, i64::from(i32::MAX)) as i32;
    Some(if horizontal {
        Action::Move { x: relative, y: 0 }
    } else {
        Action::Move { x: 0, y: relative }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    fn horizontal() -> ListLayout {
        ListLayout {
            bar: rect(0, 0, 1000, 48),
            list: rect(100, 0, 900, 48),
            first: rect(100, 4, 140, 44),
            last: rect(400, 4, 400, 44),
            previous_sibling: None,
            next_sibling: None,
            parent: Some(rect(0, 0, 1000, 48)),
        }
    }

    #[test]
    fn centers_horizontal_content() {
        assert_eq!(center(&horizontal()), Some(Action::Move { x: 350, y: 0 }));
    }

    #[test]
    fn centers_vertical_content_relative_to_parent() {
        let layout = ListLayout {
            bar: rect(0, 100, 64, 900),
            list: rect(0, 200, 64, 700),
            first: rect(4, 200, 60, 240),
            last: rect(4, 500, 60, 500),
            previous_sibling: None,
            next_sibling: None,
            parent: Some(rect(0, 100, 64, 900)),
        };
        assert_eq!(center(&layout), Some(Action::Move { x: 0, y: 250 }));
    }

    #[test]
    fn clamps_to_trailing_sibling() {
        let mut layout = horizontal();
        layout.bar = rect(100, 0, 900, 48);
        layout.parent = Some(layout.bar);
        layout.list.left = 200;
        layout.first.left = 200;
        layout.last.left = 800;
        layout.next_sibling = Some(rect(750, 0, 850, 48));
        assert_eq!(center(&layout), Some(Action::Move { x: 50, y: 0 }));
    }

    #[test]
    fn restores_instead_of_crossing_leading_boundary() {
        let mut layout = horizontal();
        layout.bar = rect(0, 0, 250, 48);
        layout.parent = Some(layout.bar);
        layout.list = rect(100, 0, 200, 48);
        assert_eq!(center(&layout), Some(Action::Restore));
    }

    #[test]
    fn avoids_redundant_move_and_restore() {
        let mut layout = horizontal();
        layout.list.left = 350;
        layout.first.left = 350;
        layout.last.left = 650;
        assert_eq!(center(&layout), Some(Action::Unchanged));

        layout = horizontal();
        layout.list.left = 340;
        layout.first.left = 350;
        layout.last.left = 650;
        assert_eq!(center(&layout), Some(Action::Unchanged));

        layout = horizontal();
        layout.bar.right = 250;
        layout.parent = Some(layout.bar);
        layout.list.left = 0;
        layout.first.left = 0;
        assert_eq!(center(&layout), Some(Action::Unchanged));
    }

    #[test]
    fn uses_physical_coordinates_on_offset_monitor() {
        let mut layout = horizontal();
        layout.bar = rect(1920, 0, 2920, 60);
        layout.parent = Some(layout.bar);
        layout.list = rect(2020, 0, 3720, 60);
        layout.first = rect(2020, 6, 2080, 54);
        layout.last = rect(2320, 6, 2320, 54);
        assert_eq!(center(&layout), Some(Action::Move { x: 350, y: 0 }));
    }
}
