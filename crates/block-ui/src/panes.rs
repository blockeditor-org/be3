use beui::Direction;
use beui::unstyled::{DockTree, DockTreeEntry, TabId};
use block_plugin_api::{MAX_PANE_DEPTH, PaneId, PaneItem, PaneTree};

pub fn pane_of(tab: TabId) -> PaneId {
    PaneId(tab.value())
}

pub fn tab_of(pane: PaneId) -> TabId {
    TabId::new(pane.0)
}

pub fn pane_tree(tree: &DockTree) -> PaneTree {
    let mut items = Vec::new();
    write(tree, &mut items);
    PaneTree { items }
}

fn write(tree: &DockTree, items: &mut Vec<PaneItem>) {
    match tree {
        DockTree::Tabs { entries, active } => {
            items.push(PaneItem::Tabs {
                count: entries.len() as u32,
                active: *active as u32,
            });
            for entry in entries {
                match entry {
                    DockTreeEntry::Tab(tab) => items.push(PaneItem::Pane(pane_of(*tab))),
                    DockTreeEntry::Group(group) => {
                        items.push(PaneItem::Group);
                        write(group, items);
                    }
                }
            }
        }
        DockTree::Split {
            direction,
            fraction,
            first,
            second,
        } => {
            items.push(PaneItem::Split {
                horizontal: *direction == Direction::Horizontal,
                fraction: *fraction,
            });
            write(first, items);
            write(second, items);
        }
    }
}

pub fn dock_tree(tree: &PaneTree) -> Option<DockTree> {
    let mut items = tree.items.iter().copied();
    let read = read(&mut items, 0)?;
    items.next().is_none().then_some(read)
}

fn read(items: &mut impl Iterator<Item = PaneItem>, depth: usize) -> Option<DockTree> {
    if depth > MAX_PANE_DEPTH {
        return None;
    }
    match items.next()? {
        PaneItem::Split {
            horizontal,
            fraction,
        } => {
            let first = read(items, depth + 1)?;
            let second = read(items, depth + 1)?;
            Some(DockTree::Split {
                direction: match horizontal {
                    true => Direction::Horizontal,
                    false => Direction::Vertical,
                },
                fraction: match fraction.is_finite() {
                    true => fraction,
                    false => 0.5,
                },
                first: Box::new(first),
                second: Box::new(second),
            })
        }
        PaneItem::Tabs { count, active } => {
            let mut entries = Vec::new();
            for _ in 0..count {
                let entry = match items.next()? {
                    PaneItem::Pane(pane) => DockTreeEntry::Tab(tab_of(pane)),
                    PaneItem::Group => DockTreeEntry::Group(read(items, depth + 1)?),
                    PaneItem::Split { .. } | PaneItem::Tabs { .. } => return None,
                };
                entries.push(entry);
            }
            Some(DockTree::Tabs {
                active: (active as usize).min(entries.len().saturating_sub(1)),
                entries,
            })
        }
        PaneItem::Pane(_) | PaneItem::Group => None,
    }
}

#[cfg(test)]
mod tests;
