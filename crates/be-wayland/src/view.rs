use std::rc::Rc;

use beui::reactive::{
    Drawing, Frame, Interactive, Portal, Prop, clone, component, component_rect, create_effect,
    create_memo, draw_gpu, in_new_scope, on_cleanup, try_with_document, view,
};
use beui::{Color32, NodeId};

use crate::state::WindowId;
use crate::windows::{Fullscreen, Insets, Windows};

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
        let away = fullscreen.get().is_some_and(|fullscreen| fullscreen.id == id);
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
    let fullscreen = windows.fullscreen();
    let capture = create_memo(move || fullscreen.get().is_some_and(|shown| shown.id == id));
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
            capture_presses={capture}
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
    let inset = |side: fn(Insets) -> f32| {
        let fullscreen = fullscreen.clone();
        create_memo(move || {
            fullscreen
                .get()
                .map(|Fullscreen { insets, .. }| side(insets))
        })
    };
    let left = inset(|insets| insets.left);
    let top = inset(|insets| insets.top);
    let right = inset(|insets| insets.right);
    let bottom = inset(|insets| insets.bottom);
    view! {
        <Frame
            @test_id="wayland.fullscreen"
            visible={shown}
            padding_left={left}
            padding_top={top}
            padding_right={right}
            padding_bottom={bottom}
        >
            <Frame color=Color32::BLACK>
                <Portal node />
            </Frame>
        </Frame>
    }
}
