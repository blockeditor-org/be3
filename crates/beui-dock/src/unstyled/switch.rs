use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::KeyEvent;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Key;
use beui_view::types::Placement;
use beui_view::types::Pos2;
use beui_view::types::icons::Glyph;
use beui_view::types::{Dismiss, LayerPolicy};

use super::{DockState, State, TabId};

pub struct DockSwitchHandle {
    pub tabs: Memo<Vec<TabId>>,
    pub chosen: Memo<Option<TabId>>,
    pub row: Func<TabId, DockSwitchRowHandle>,
}

pub struct DockSwitchRowHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
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
    let row = Func::new(move |tab: TabId| DockSwitchRowHandle {
        tab,
        title: create_memo(move || dock.title(tab)),
        icon: create_memo(move || dock.icon(tab)),
        chosen: create_memo(move || chosen.get() == Some(tab)),
        pick: ClickCallback::new(move || dock.pick_switch(tab)),
    });
    let body = move || match &dock.switch {
        Some(render) => render.call(DockSwitchHandle { tabs, chosen, row }),
        None => view! {
            <Frame />
        },
    };
    let overlay = NodeRef::new();
    on_global_key(move |event: KeyEvent| {
        if event.held() {
            return true;
        }
        let Some(press) = event.press() else {
            return false;
        };
        let Some(overlay) = overlay.try_get() else {
            return false;
        };
        if press.key != Key::Escape || !press.pressed || !open.get_untracked() {
            return false;
        }
        overlay.dismiss_overlay(Dismiss::Escape);
        true
    });
    view! {
        <Overlay
            @node_ref=&overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::Center
            policy=LayerPolicy::Pass
            traps_focus=false
            open
            on_dismiss={move |_: Dismiss| dock.edit(DockState::cancel_switch)}
        >
            <Frame @test_id="dock.switch">{body()}</Frame>
        </Overlay>
    }
}
