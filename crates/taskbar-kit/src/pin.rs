#[derive(Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub name: String,
}

impl Item {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Move configured items to the head and preserve every other item's order.
pub fn pin_order(items: &[Item], pin: &[String]) -> Vec<String> {
    let mut head = Vec::new();
    for configured in pin {
        if let Some(item) = items
            .iter()
            .find(|item| matches(item, configured) && !head.contains(&item.id))
        {
            head.push(item.id.clone());
        }
    }
    head.into_iter()
        .chain(
            items
                .iter()
                .filter(|item| !pin.iter().any(|name| matches(item, name)))
                .map(|item| item.id.clone()),
        )
        .collect()
}

fn matches(item: &Item, configured: &str) -> bool {
    let id = item
        .id
        .split_once('|')
        .map_or(item.id.as_str(), |(id, _)| id);
    let file = id.rsplit(['\\', '/']).next().unwrap_or(id);
    let stem = file
        .len()
        .checked_sub(4)
        .and_then(|index| match (file.get(..index), file.get(index..)) {
            (Some(stem), Some(extension)) if extension.eq_ignore_ascii_case(".exe") => Some(stem),
            _ => None,
        })
        .unwrap_or(file);
    item.id.eq_ignore_ascii_case(configured)
        || id.eq_ignore_ascii_case(configured)
        || item.name.eq_ignore_ascii_case(configured)
        || stem.eq_ignore_ascii_case(configured)
}

/// Compute one drag per step, robust to Explorer inserting/removing buttons.
pub fn moves_to(current: &[String], target: &[String]) -> Vec<(String, usize)> {
    let mut work = current.to_vec();
    let mut moves = Vec::new();
    if work.len() != target.len() || work.iter().any(|id| !target.contains(id)) {
        return moves;
    }
    for index in 0..target.len() {
        if work[index] == target[index] {
            continue;
        }
        let Some(old) = work.iter().position(|id| id == &target[index]) else {
            continue;
        };
        let id = work.remove(old);
        work.insert(index, id.clone());
        moves.push((id, index));
    }
    moves
}

pub fn first_move(current: &[String], target: &[String]) -> Option<(String, usize)> {
    moves_to(current, target).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn pins_present_items_and_preserves_other_order() {
        let items = vec![
            Item::new("z-id", "Zulu"),
            Item::new("net", "Network"),
            Item::new("a-id", "alpha"),
            Item::new("m-id", "Mike"),
        ];
        let pin = ids(&["Mike", "z-id", "missing", "Mike"]);
        assert_eq!(
            pin_order(&items, &pin),
            ids(&["m-id", "z-id", "net", "a-id"])
        );
        assert_eq!(
            pin_order(&items, &[]),
            items.iter().map(|i| i.id.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pins_match_executable_stems_case_insensitively() {
        let items = vec![
            Item::new("Brave|Brave - 1 window", "Brave - 1 window"),
            Item::new(
                r"C:\apps\DoubleCmd\doublecmd.EXE|Double Commander",
                "Double Commander",
            ),
            Item::new(
                r"C:\apps\Sublime\sublime_text.exe|Sublime Text",
                "Sublime Text",
            ),
            Item::new(r"C:\apps\WezTerm\wezterm-gui.exe|WezTerm", "WezTerm"),
        ];
        let pin = ids(&["brave", "doublecmd", "sublime_text"]);
        assert_eq!(
            pin_order(&items, &pin),
            ids(&[
                items[0].id.as_str(),
                items[1].id.as_str(),
                items[2].id.as_str(),
                items[3].id.as_str()
            ])
        );
    }

    #[test]
    fn move_sequence_converges() {
        let current = ids(&["a", "b", "c", "d"]);
        let target = ids(&["d", "b", "a", "c"]);
        let mut actual = current;
        for (id, index) in moves_to(&actual, &target) {
            let old = actual.iter().position(|item| item == &id).unwrap();
            actual.remove(old);
            actual.insert(index, id);
        }
        assert_eq!(actual, target);
        assert_eq!(first_move(&actual, &target), None);
    }

    #[test]
    fn membership_change_needs_fresh_target() {
        assert!(moves_to(&ids(&["a", "b"]), &ids(&["a"])).is_empty());
    }
}
