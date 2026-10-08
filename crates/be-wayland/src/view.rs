use std::rc::Rc;

use beui::reactive::{
    Drawing, Frame, Interactive, Overlay, OverlayAnchor, OverlayMode, Placement, Portal, Prop,
    clone, component, component_rect, create_effect, create_memo, draw_gpu, in_new_scope,
    on_cleanup, try_with_document, view,
};
use beui::{Color32, NodeId, Rect};

use crate::state::WindowId;
use crate::windows::Windows;

#[component]
pub fn WindowView(windows: Windows, id: WindowId) -> NodeId {
    let Some(signals) = windows.signals(id) else {
        return view! {
            <Frame />
        };
    };
    let content = in_new_scope(clone!(windows -> move || view! {
        <WindowContent windows id />
    }));
    signals.shown_by(Some(content));
    on_cleanup(move || {
        signals.unshown(content);
        try_with_document(|document| document.remove_node(content));
    });
    let fullscreen = windows.fullscreen();
    let node = create_memo(move || {
        let away = fullscreen
            .get()
            .is_some_and(|fullscreen| fullscreen.id == id);
        (!away).then_some(content)
    });
    view! {
        <Portal node />
    }
}

#[component]
fn WindowContent(windows: Windows, id: WindowId) -> NodeId {
    let Some(signals) = windows.signals(id) else {
        return view! {
            <Frame />
        };
    };
    let rect = component_rect();
    create_effect(clone!(windows -> move || windows.placed(id, rect.get())));
    let cursor = windows.cursor();
    let focus = windows.clone();
    let hover = windows.clone();
    on_cleanup(move || windows.unplaced(id));
    view! {
        <Interactive
            focusable=true
            @test_id={format!("wayland.window.{}", id.0)}
            focused={signals.focused}
            on_focus_change={move |focused: bool| focus.focus(id, focused)}
            on_key={|_| true}
            cursor
            on_hover_change={move |hovered: bool| hover.hover(id, hovered)}
        >
            <Drawing draw={Prop::Dynamic(Rc::new(move || draw_gpu(signals.drawing.get())))} />
        </Interactive>
    }
}

#[component]
pub fn FullscreenWindow(windows: Windows) -> NodeId {
    let fullscreen = windows.fullscreen();
    let node = create_memo(clone!(fullscreen -> move || {
        let id = fullscreen.get()?.id;
        windows.signals(id)?.node.get()
    }));
    let shown = create_memo(clone!(node -> move || node.get().is_some()));
    let area = create_memo(move || fullscreen.get().map_or(Rect::ZERO, |shown| shown.area));
    let anchor = create_memo(clone!(area -> move || OverlayAnchor::Point(area.get().min)));
    let width = create_memo(clone!(area -> move || Some(area.get().width())));
    let height = create_memo(move || Some(area.get().height()));
    view! {
        <Overlay
            anchor
            placement=Placement::At
            mode=OverlayMode::Floating
            traps_focus=false
            open={shown}
        >
            <Frame @test_id="wayland.fullscreen" width height color=Color32::BLACK>
                <Portal node />
            </Frame>
        </Overlay>
    }
}
