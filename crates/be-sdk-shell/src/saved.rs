use std::collections::HashMap;

use be_sdk_beui::SavedDockLayout;
use be_sdk_beui::be_block_content::{EditorView, ViewRef, ViewStateKind};
use be_sdk_beui::beui::dock::{DockingSnapshot, TabId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::program::ProgramRecord;
use super::tab::TabItem;

/// The session's dock, its tabs with their view blocks, and its programs.
#[derive(Deserialize, PartialEq, Serialize)]
pub struct SavedLayout {
    dock: SavedDockLayout,
    next_tab: u64,
    tabs: Vec<SavedTab>,
    programs: Vec<SavedProgram>,
}

impl ViewStateKind for SavedLayout {
    const ID: Uuid = Uuid::from_u128(0x7368_656c_6c2d_6c61_796f_7574_0000_0001);
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SavedProgram {
    pub tab: u64,
    pub record: ProgramRecord,
}

#[derive(Deserialize, PartialEq, Serialize)]
struct SavedTab {
    tab: u64,
    view: ViewRef,
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
) -> SavedLayout {
    let mut open: Vec<(&TabId, &TabItem)> = tabs.iter().collect();
    open.sort_by_key(|(tab, _)| **tab);
    let tabs = open
        .into_iter()
        .filter_map(|(tab, item)| {
            Some(SavedTab {
                tab: tab.value(),
                view: ViewRef::new(*views.get(tab)?),
                block: item.id,
                block_type: item.block_type,
                editor: item.editor,
            })
        })
        .collect();
    SavedLayout {
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
    }
}

/// The programs a session's view keeps in its layout.
pub fn saved_programs(view: &EditorView) -> Vec<SavedProgram> {
    view.view_state::<SavedLayout>()
        .and_then(Result::ok)
        .map(|saved| saved.programs)
        .unwrap_or_default()
}

/// The layout as the session shows it; a tab whose view was deleted since is left out.
pub(crate) fn restore(saved: SavedLayout) -> Restored {
    let tabs = saved
        .tabs
        .iter()
        .filter_map(|tab| {
            let item = TabItem::new(tab.block, tab.block_type).edited_with(tab.editor);
            Some((TabId::new(tab.tab), (item, tab.view.block()?)))
        })
        .collect();
    Restored {
        dock: saved.dock.snapshot(),
        next_tab: saved.next_tab,
        tabs,
        programs: saved
            .programs
            .into_iter()
            .map(|saved| (TabId::new(saved.tab), saved.record))
            .collect(),
    }
}

/// The tabs' titles, shown for a reopened tab before its block is known.
#[derive(Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct SavedTitles(Vec<(u64, String)>);

impl ViewStateKind for SavedTitles {
    const ID: Uuid = Uuid::from_u128(0x7368_656c_6c2d_7469_746c_6573_0000_0001);
}

impl SavedTitles {
    pub(crate) fn of(titles: &HashMap<TabId, String>) -> Self {
        let mut saved: Vec<(u64, String)> = titles
            .iter()
            .map(|(tab, title)| (tab.value(), title.clone()))
            .collect();
        saved.sort();
        Self(saved)
    }

    pub(crate) fn titles(self) -> HashMap<TabId, String> {
        self.0
            .into_iter()
            .map(|(tab, title)| (TabId::new(tab), title))
            .collect()
    }
}

/// The views of the tabs closed lately, most recent first: a block opened again in the same
/// editor gets its view back, with whatever the editor kept in it (where it was scrolled to).
#[derive(Default, Deserialize, PartialEq, Serialize)]
pub struct ClosedViews(pub(crate) Vec<ClosedView>);

impl ViewStateKind for ClosedViews {
    const ID: Uuid = Uuid::from_u128(0x7368_656c_6c2d_636c_6f73_6564_0000_0001);
}

pub(crate) const MAX_CLOSED_VIEWS: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct ClosedView {
    pub(crate) block: Uuid,
    pub(crate) editor: Uuid,
    pub(crate) view: ViewRef,
}
