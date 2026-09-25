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
    pane_tree_with(tree, &|tab| Some(pane_of(tab)))
}

pub fn pane_tree_with(tree: &DockTree, pane: &dyn Fn(TabId) -> Option<PaneId>) -> PaneTree {
    let mut items = Vec::new();
    write(tree, pane, &mut items);
    PaneTree { items }
}

fn write(tree: &DockTree, pane: &dyn Fn(TabId) -> Option<PaneId>, items: &mut Vec<PaneItem>) {
    match tree {
        DockTree::Tabs { entries, active } => {
            let kept: Vec<&DockTreeEntry> = entries
                .iter()
                .filter(|entry| match entry {
                    DockTreeEntry::Tab(tab) => pane(*tab).is_some(),
                    DockTreeEntry::Group(_) => true,
                })
                .collect();
            let active = entries
                .get(*active)
                .and_then(|shown| kept.iter().position(|entry| *entry == shown))
                .unwrap_or(0);
            items.push(PaneItem::Tabs {
                count: kept.len() as u32,
                active: active as u32,
            });
            for entry in kept {
                match entry {
                    DockTreeEntry::Tab(tab) => {
                        if let Some(pane) = pane(*tab) {
                            items.push(PaneItem::Pane(pane));
                        }
                    }
                    DockTreeEntry::Group(group) => {
                        items.push(PaneItem::Group);
                        write(group, pane, items);
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
            write(first, pane, items);
            write(second, pane, items);
        }
    }
}

pub fn dock_tree(tree: &PaneTree) -> Option<DockTree> {
    dock_tree_with(tree, &tab_of)
}

pub fn dock_tree_with(tree: &PaneTree, tab: &dyn Fn(PaneId) -> TabId) -> Option<DockTree> {
    let mut items = tree.items.iter().copied();
    let read = read(&mut items, tab, 0)?;
    items.next().is_none().then_some(read)
}

fn read(
    items: &mut impl Iterator<Item = PaneItem>,
    tab: &dyn Fn(PaneId) -> TabId,
    depth: usize,
) -> Option<DockTree> {
    if depth > MAX_PANE_DEPTH {
        return None;
    }
    match items.next()? {
        PaneItem::Split {
            horizontal,
            fraction,
        } => {
            let first = read(items, tab, depth + 1)?;
            let second = read(items, tab, depth + 1)?;
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
                    PaneItem::Pane(pane) => DockTreeEntry::Tab(tab(pane)),
                    PaneItem::Group => DockTreeEntry::Group(read(items, tab, depth + 1)?),
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
