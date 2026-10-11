use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::geometry::Pos2;
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    ClickCallback, Frame, Interactive, Memo, Portal, clone, create_effect, create_memo,
    create_signal, untrack, with_document,
};

use super::{DockState, Handle, TabId};

pub struct DockAwayHandle {
    pub title: Memo<String>,
    pub icon: Memo<String>,
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
        let state = self.state.clone();
        create_memo(move || {
            let away = state.with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab));
            shown.get().filter(|tab| away == Some(*tab))
        })
    }
}

#[component]
pub(super) fn DockFullscreenView(dock: Handle) -> NodeId {
    let state = dock.state.clone();
    let fullscreen = create_memo(move || state.with(DockState::fullscreen));
    let (panel, set_panel) = create_signal(None);
    create_effect(clone!(dock fullscreen -> move || {
        set_panel.set(fullscreen.get().map(|shown| dock.panel(shown.tab)));
    }));
    let open = create_memo(clone!(fullscreen -> move || fullscreen.get().is_some()));
    let area = create_memo(move || fullscreen.get().and_then(|shown| shown.area));
    let placement = create_memo(clone!(area -> move || match area.get() {
        Some(_) => Placement::At,
        None => Placement::FillScreen,
    }));
    let anchor = create_memo(clone!(area -> move || {
        OverlayAnchor::Point(area.get().map_or(Pos2::ZERO, |area| area.min))
    }));
    let width = create_memo(clone!(area -> move || area.get().map(|area| area.width())));
    let height = create_memo(move || area.get().map(|area| area.height()));
    let overlay = dock.fullscreen_overlay.clone();
    let front = create_memo(clone!(dock -> move || dock.fullscreen_in_front()));
    create_effect(clone!(overlay -> move || {
        if !front.get() {
            return;
        }
        if let Some(overlay) = overlay.try_get() {
            untrack(|| with_document(|document| document.raise_overlay(overlay)));
        }
    }));
    let pressed = dock.clone();
    let body = dock.fullscreen.call(view! {
        <Portal node={panel} />
    });
    view! {
        <Overlay
            @node_ref=&overlay
            anchor
            placement
            mode=OverlayMode::Floating
            traps_focus=false
            open
        >
            <Interactive
                on_press={move |_: PointerPress| {
                    if let Some(tab) = pressed.fullscreen_tab() {
                        pressed.show(tab);
                        pressed.release_dialog_focus();
                    }
                }}
            >
                <Frame @test_id="dock.fullscreen" width height>{body}</Frame>
            </Interactive>
        </Overlay>
    }
}

#[component]
pub(super) fn DockAway(dock: Handle, gone: Memo<Option<TabId>>) -> NodeId {
    let titled = dock.clone();
    let title = create_memo(clone!(gone -> move || {
        gone.get().map(|tab| titled.title(tab)).unwrap_or_default()
    }));
    let named = dock.clone();
    let icon = create_memo(clone!(gone -> move || {
        gone.get().map(|tab| named.icon(tab)).unwrap_or_default()
    }));
    let leaving = dock.clone();
    let leave = ClickCallback::new(move || {
        if let Some(tab) = gone.get_untracked() {
            leaving.leave_fullscreen(tab);
        }
    });
    dock.away.call(DockAwayHandle { title, icon, leave })
}
