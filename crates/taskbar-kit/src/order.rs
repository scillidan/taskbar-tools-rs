//! Platform-independent ordering shared by taskbar and notification-area tools.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    #[default]
    Alphabetical,
    AlphabeticalDesc,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    /// Stable Explorer/UIA identity when available; otherwise the display name.
    pub id: String,
    pub name: String,
    pub system: bool,
}

impl Item {
    pub fn new(id: impl Into<String>, name: impl Into<String>, system: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            system,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct OrderConfig {
    pub mode: SortMode,
    pub lock_head: Vec<String>,
    pub lock_tail: Vec<String>,
    pub system_names: Vec<String>,
}

/// Return IDs in display order. System items form the final group even when
/// user applications are locked to the tail.
pub fn planned_order(items: &[Item], cfg: &OrderConfig, custom: &[String]) -> Vec<String> {
    let by_id: HashMap<&str, &Item> = items.iter().map(|i| (i.id.as_str(), i)).collect();
    let head = locked(&cfg.lock_head, &by_id);
    let tail = locked(&cfg.lock_tail, &by_id);
    let custom_rank: HashMap<&str, usize> = custom
        .iter()
        .enumerate()
        .map(|(n, id)| (id.as_str(), n))
        .collect();

    let mut middle: Vec<&Item> = items
        .iter()
        .filter(|item| !head.contains(&item.id) && !tail.contains(&item.id))
        .collect();
    middle.sort_by(|a, b| {
        let system = a.system.cmp(&b.system);
        if system != Ordering::Equal {
            return system;
        }
        let order = match cfg.mode {
            SortMode::Alphabetical => natural_name(a).cmp(&natural_name(b)),
            SortMode::AlphabeticalDesc => natural_name(b).cmp(&natural_name(a)),
            SortMode::Custom => custom_rank
                .get(a.id.as_str())
                .unwrap_or(&usize::MAX)
                .cmp(custom_rank.get(b.id.as_str()).unwrap_or(&usize::MAX)),
        };
        order
            .then_with(|| natural_name(a).cmp(&natural_name(b)))
            .then_with(|| a.id.cmp(&b.id))
    });

    let system_start = middle
        .iter()
        .position(|item| item.system)
        .unwrap_or(middle.len());
    let systems = middle.split_off(system_start);
    head.into_iter()
        .chain(middle.into_iter().map(|i| i.id.clone()))
        .chain(tail)
        .chain(systems.into_iter().map(|i| i.id.clone()))
        .collect()
}

fn locked(wanted: &[String], by_id: &HashMap<&str, &Item>) -> Vec<String> {
    wanted
        .iter()
        .filter_map(|name| {
            by_id
                .values()
                .find(|item| !item.system && matches(item, name))
                .map(|item| item.id.clone())
        })
        .collect()
}

fn matches(item: &Item, configured: &str) -> bool {
    item.id.eq_ignore_ascii_case(configured) || item.name.eq_ignore_ascii_case(configured)
}

fn natural_name(item: &Item) -> String {
    item.name.to_lowercase()
}

/// Minimal sequential moves: move `id` so it lands at `index` in the current
/// list. The caller re-enumerates between physical drag operations.
pub fn moves_to(current: &[String], target: &[String]) -> Vec<(String, usize)> {
    if current.len() != target.len()
        || !current.iter().all(|id| target.contains(id))
        || !target.iter().all(|id| current.contains(id))
    {
        return Vec::new();
    }
    let mut work = current.to_vec();
    let mut moves = Vec::new();
    for index in 0..target.len() {
        if work[index] == target[index] {
            continue;
        }
        let id = target[index].clone();
        let old = work.iter().position(|other| other == &id).unwrap();
        work.remove(old);
        work.insert(index, id.clone());
        moves.push((id, index));
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<Item> {
        vec![
            Item::new("z-id", "Zulu", false),
            Item::new("net", "Network", true),
            Item::new("a-id", "alpha", false),
            Item::new("m-id", "Mike", false),
        ]
    }

    #[test]
    fn alphabetical_locks_and_system_tail() {
        let cfg = OrderConfig {
            lock_head: vec!["Mike".into()],
            lock_tail: vec!["z-id".into()],
            ..Default::default()
        };
        assert_eq!(
            planned_order(&items(), &cfg, &[]),
            ["m-id", "a-id", "z-id", "net"]
        );
    }

    #[test]
    fn reverse_keeps_system_last() {
        let cfg = OrderConfig {
            mode: SortMode::AlphabeticalDesc,
            ..Default::default()
        };
        assert_eq!(
            planned_order(&items(), &cfg, &[]),
            ["z-id", "m-id", "a-id", "net"]
        );
    }

    #[test]
    fn custom_remembers_known_and_appends_new() {
        let cfg = OrderConfig {
            mode: SortMode::Custom,
            ..Default::default()
        };
        let custom = vec!["m-id".into(), "z-id".into(), "net".into()];
        assert_eq!(
            planned_order(&items(), &cfg, &custom),
            ["m-id", "z-id", "a-id", "net"]
        );
    }

    #[test]
    fn moves_are_sequential_and_reject_different_sets() {
        let current = vec!["a".into(), "b".into(), "c".into()];
        let target = vec!["c".into(), "a".into(), "b".into()];
        assert_eq!(moves_to(&current, &target), vec![("c".into(), 0)]);
        assert!(moves_to(&current, &["a".into()]).is_empty());
    }
}
