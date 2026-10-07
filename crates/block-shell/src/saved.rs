use std::collections::HashMap;

use block_editor_beui::be_block::ViewState;
use block_editor_beui::beui::unstyled::{DockingSnapshot, TabId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::tab::TabItem;

pub(crate) const LAYOUT: &str = "layout";

#[derive(Deserialize, Serialize)]
struct SavedLayout {
    dock: DockingSnapshot<TabId>,
    next_tab: u64,
    files: usize,
    tabs: Vec<SavedTab>,
}

#[derive(Deserialize, Serialize)]
struct SavedTab {
    tab: u64,
    view: usize,
    block: Uuid,
    block_type: Uuid,
}

pub(crate) struct Restored {
    pub(crate) dock: DockingSnapshot<TabId>,
    pub(crate) next_tab: u64,
    pub(crate) files: Option<Uuid>,
    pub(crate) tabs: HashMap<TabId, (TabItem, Uuid)>,
}

pub(crate) fn save(
    dock: &DockingSnapshot<TabId>,
    next_tab: u64,
    files: Option<Uuid>,
    tabs: &HashMap<TabId, TabItem>,
    views: &HashMap<TabId, Uuid>,
) -> ViewState {
    let mut refs = vec![files.unwrap_or_else(Uuid::nil)];
    let mut open: Vec<(&TabId, &TabItem)> = tabs.iter().collect();
    open.sort_by_key(|(tab, _)| **tab);
    let tabs = open
        .into_iter()
        .filter_map(|(tab, item)| {
            refs.push(*views.get(tab)?);
            Some(SavedTab {
                tab: tab.value(),
                view: refs.len() - 1,
                block: item.id,
                block_type: item.block_type,
            })
        })
        .collect();
    let saved = SavedLayout {
        dock: dock.clone(),
        next_tab,
        files: 0,
        tabs,
    };
    ViewState::new(&saved, refs)
}

pub(crate) fn restore(state: &ViewState) -> Option<Restored> {
    let saved: SavedLayout = state.value()?;
    let referenced = |index: usize| state.refs.get(index).copied().filter(|id| !id.is_nil());
    let tabs = saved
        .tabs
        .iter()
        .filter_map(|tab| {
            let item = TabItem {
                id: tab.block,
                block_type: tab.block_type,
            };
            Some((TabId::new(tab.tab), (item, referenced(tab.view)?)))
        })
        .collect();
    Some(Restored {
        dock: saved.dock,
        next_tab: saved.next_tab,
        files: referenced(saved.files),
        tabs,
    })
}
