use beui_macros::component;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::icons::Glyph;

use super::{State, TabId};

pub struct DockAwayHandle {
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub leave: ClickCallback,
}

impl super::State {
    pub(super) fn fullscreen_tab(&self) -> Option<TabId> {
        self.state
            .with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab))
    }

    pub(super) fn fullscreen_in_front(&self) -> bool {
        self.state.with(|state| {
            state
                .fullscreen()
                .is_some_and(|fullscreen| state.focused_tab() == Some(fullscreen.tab))
        })
    }

    pub(super) fn offers_fullscreen(&self, tab: TabId) -> bool {
        self.offers.with(|offers| offers.contains_key(&tab))
    }

    pub(super) fn away_from(&self, shown: Memo<Option<TabId>>) -> Memo<Option<TabId>> {
        let dock = *self;
        create_memo(move || {
            let away = dock.fullscreen_tab();
            shown.get().filter(|tab| away == Some(*tab))
        })
    }
}

#[component]
pub(super) fn DockAway(dock: State, gone: Memo<Option<TabId>>) -> NodeId {
    let title = create_memo(move || gone.get().map(|tab| dock.title(tab)).unwrap_or_default());
    let icon = create_memo(move || gone.get().map(|tab| dock.icon(tab)).unwrap_or_default());
    let leave = ClickCallback::new(move || {
        if let Some(tab) = gone.get_untracked() {
            dock.leave_fullscreen(tab);
        }
    });
    dock.away.call(DockAwayHandle { title, icon, leave })
}
