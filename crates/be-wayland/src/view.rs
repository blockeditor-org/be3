use std::rc::Rc;
use std::time::Duration;

use beui::reactive::{
    Action, Chord, Drawing, Frame, Interactive, Layers, Prop, clone, component, component_rect,
    create_effect, create_memo, draw_gpu, on_cleanup, try_with_document, view,
};
use beui::{Color32, Key, NodeId, icons};

use crate::state::WindowId;
use crate::windows::Windows;

const FROZEN_DIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 140);

pub fn toggle_fullscreen_action(windows: &Windows) -> Action {
    let windows = windows.clone();
    Action::new("wayland.fullscreen", "Toggle fullscreen", move || {
        windows.toggle_fullscreen();
        try_with_document(|document| document.request_repaint_after(Duration::ZERO));
    })
    .glyph(icons::ICON_FULLSCREEN)
    .shortcut(Chord::logo(Key::F))
    .intercepts()
    .register()
}

#[component]
pub fn WindowView(windows: Windows, id: WindowId) -> NodeId {
    let Some(signals) = windows.signals(id) else {
        return view! {
            <Frame />
        };
    };
    let rect = component_rect();
    create_effect(clone!(windows -> move || windows.placed(id, rect.get())));
    let list = windows.list();
    let dim = create_memo(move || {
        let responding = list.with(|list| {
            list.iter()
                .find(|info| info.id == id)
                .is_none_or(|info| info.responding)
        });
        match responding {
            true => Color32::TRANSPARENT,
            false => FROZEN_DIM,
        }
    });
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
            <Layers>
                <Drawing draw={Prop::Dynamic(Rc::new(move || draw_gpu(signals.drawing.get())))} />
                <Frame color={dim} />
            </Layers>
        </Interactive>
    }
}
