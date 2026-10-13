use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Pos2;
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{ClickCallback, Frame, Func, Memo, create_memo};

use super::{DockState, State, TabId};

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
pub(super) fn DockSwitchView(dock: State) -> NodeId {
    let state = dock.state;
    let switch = create_memo(move || state.with(DockState::switch));
    let open = create_memo(move || switch.with(Option::is_some));
    let tabs = create_memo(move || {
        switch.with(|switch| {
            switch
                .as_ref()
                .map(|switch| switch.tabs.clone())
                .unwrap_or_default()
        })
    });
    let chosen =
        create_memo(move || switch.with(|switch| switch.as_ref().map(|switch| switch.chosen)));
    let rows = dock;
    let row = Func::new(move |tab: TabId| {
        let (titled, pictured, picked) = (rows.clone(), rows.clone(), rows.clone());
        DockSwitchRowHandle {
            tab,
            title: create_memo(move || titled.title(tab)),
            icon: create_memo(move || pictured.icon(tab)),
            chosen: create_memo(move || chosen.get() == Some(tab)),
            pick: ClickCallback::new(move || picked.pick_switch(tab)),
        }
    });
    let body = match &dock.switch {
        Some(render) => render.call(DockSwitchHandle { tabs, chosen, row }),
        None => view! {
            <Frame />
        },
    };
    let cancelled = dock;
    view! {
        <Overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::Center
            policy=LayerPolicy::Pass
            traps_focus=false
            open
            on_dismiss={move |_: Dismiss| cancelled.edit(DockState::cancel_switch)}
        >
            <Frame @test_id="dock.switch">{body}</Frame>
        </Overlay>
    }
}
