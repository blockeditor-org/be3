use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{ClickCallback, Frame, Func, Memo, clone, create_memo};

use super::{DockState, Handle, State, TabId};

pub struct DockSwitchHandle {
    pub tabs: Memo<Vec<TabId>>,
    pub chosen: Memo<Option<TabId>>,
    pub row: Func<TabId, DockSwitchRowHandle>,
}

pub struct DockSwitchRowHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    pub icon: Memo<String>,
    pub chosen: Memo<bool>,
    pub pick: ClickCallback,
}

impl State {
    fn pick_switch(&self, tab: TabId) {
        self.edit(|state| {
            state.choose_switch(tab);
            state.commit_switch();
        });
    }
}

#[component]
pub(super) fn DockSwitchView(dock: Handle) -> NodeId {
    let state = dock.state.clone();
    let switch = create_memo(move || state.with(DockState::switch));
    let open = create_memo(clone!(switch -> move || switch.with(Option::is_some)));
    let tabs = create_memo(clone!(switch -> move || {
        switch.with(|switch| switch.as_ref().map(|switch| switch.tabs.clone()).unwrap_or_default())
    }));
    let chosen =
        create_memo(move || switch.with(|switch| switch.as_ref().map(|switch| switch.chosen)));
    let rows = dock.clone();
    let row = Func::new(clone!(chosen -> move |tab: TabId| {
        let (titled, pictured, picked) = (rows.clone(), rows.clone(), rows.clone());
        DockSwitchRowHandle {
            tab,
            title: create_memo(move || titled.title(tab)),
            icon: create_memo(move || pictured.icon(tab)),
            chosen: create_memo(clone!(chosen -> move || chosen.get() == Some(tab))),
            pick: ClickCallback::new(move || picked.pick_switch(tab)),
        }
    }));
    let body = match &dock.switch {
        Some(render) => render.call(DockSwitchHandle { tabs, chosen, row }),
        None => view! {
            <Frame />
        },
    };
    let cancelled = dock.clone();
    view! {
        <Overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::Center
            mode=OverlayMode::Floating
            traps_focus=false
            open
            on_dismiss={move || cancelled.edit(DockState::cancel_switch)}
        >
            <Frame @test_id="dock.switch">{body}</Frame>
        </Overlay>
    }
}
