use std::collections::HashMap;

use be_sdk_beui::SavedDockLayout;
use be_sdk_beui::be_block::ViewState;
use be_sdk_beui::beui::dock::{DockingSnapshot, TabId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::program::ProgramRecord;
use super::tab::TabItem;

pub(crate) const LAYOUT: &str = "layout";
pub(crate) const TITLES: &str = "titles";

#[derive(Deserialize, Serialize)]
struct SavedLayout {
    dock: SavedDockLayout,
    next_tab: u64,
    tabs: Vec<SavedTab>,
    programs: Vec<SavedProgram>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SavedProgram {
    pub tab: u64,
    pub record: ProgramRecord,
}

#[derive(Deserialize, Serialize)]
struct SavedTab {
    tab: u64,
    view: usize,
    block: Uuid,
    block_type: Uuid,
    editor: Option<Uuid>,
}

pub(crate) struct Restored {
    pub(crate) dock: DockingSnapshot<TabId>,
    pub(crate) next_tab: u64,
    pub(crate) tabs: HashMap<TabId, (TabItem, Uuid)>,
    pub(crate) programs: Vec<(TabId, ProgramRecord)>,
}

pub(crate) fn save(
    dock: &DockingSnapshot<TabId>,
    next_tab: u64,
    tabs: &HashMap<TabId, TabItem>,
    views: &HashMap<TabId, Uuid>,
    programs: Vec<(TabId, ProgramRecord)>,
) -> ViewState {
    let mut refs = Vec::new();
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
                editor: item.editor,
            })
        })
        .collect();
    let saved = SavedLayout {
        dock: SavedDockLayout::of(dock),
        next_tab,
        tabs,
        programs: programs
            .into_iter()
            .map(|(tab, record)| SavedProgram {
                tab: tab.value(),
                record,
            })
            .collect(),
    };
    ViewState::new(&saved, refs)
}

pub fn saved_programs(state: &ViewState) -> Vec<SavedProgram> {
    state
        .value::<SavedLayout>()
        .map(|saved| saved.programs)
        .unwrap_or_default()
}

pub(crate) fn restore(state: &ViewState) -> Result<Restored, String> {
    let saved: SavedLayout = state.read().map_err(|error| error.to_string())?;
    let referenced = |index: usize| state.refs.get(index).copied().filter(|id| !id.is_nil());
    let tabs = saved
        .tabs
        .iter()
        .filter_map(|tab| {
            let item = TabItem::new(tab.block, tab.block_type).edited_with(tab.editor);
            Some((TabId::new(tab.tab), (item, referenced(tab.view)?)))
        })
        .collect();
    Ok(Restored {
        dock: saved.dock.snapshot(),
        next_tab: saved.next_tab,
        tabs,
        programs: saved
            .programs
            .into_iter()
            .map(|saved| (TabId::new(saved.tab), saved.record))
            .collect(),
    })
}

pub(crate) fn save_titles(titles: &HashMap<TabId, String>) -> ViewState {
    let mut saved: Vec<(u64, &String)> = titles
        .iter()
        .map(|(tab, title)| (tab.value(), title))
        .collect();
    saved.sort();
    ViewState::new(&saved, Vec::new())
}

pub(crate) fn restore_titles(state: &ViewState) -> HashMap<TabId, String> {
    state
        .value::<Vec<(u64, String)>>()
        .unwrap_or_default()
        .into_iter()
        .map(|(tab, title)| (TabId::new(tab), title))
        .collect()
}

/// The views of the tabs closed lately, most recent first: a block opened again in the same
/// editor gets its view back, with whatever the editor kept in it (where it was scrolled to).
pub(crate) const CLOSED_VIEWS: &str = "closed-views";
pub(crate) const MAX_CLOSED_VIEWS: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub(crate) struct ClosedView {
    pub(crate) block: Uuid,
    pub(crate) editor: Uuid,
}

/// The closed views in `state`, each with its view block (the ref of the same index); a view
/// deleted since is blanked, and left out.
pub(crate) fn closed_views(state: Option<&ViewState>) -> Vec<(ClosedView, Uuid)> {
    let Some(state) = state else {
        return Vec::new();
    };
    let listed: Vec<ClosedView> = state.value().unwrap_or_default();
    listed
        .into_iter()
        .zip(state.refs.iter().copied())
        .filter(|(_, view)| !view.is_nil())
        .collect()
}

pub(crate) fn save_closed_views(closed: Vec<(ClosedView, Uuid)>) -> ViewState {
    let (listed, refs): (Vec<ClosedView>, Vec<Uuid>) = closed.into_iter().unzip();
    ViewState::new(&listed, refs)
}
