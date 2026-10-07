use std::rc::Rc;

use beui::NodeId;
use beui::reactive::{
    Drawing, Frame, Interactive, Prop, clone, component, component_rect, create_effect, draw_gpu,
    on_cleanup, view,
};

use crate::state::WindowId;
use crate::windows::Windows;

#[component]
pub fn WindowView(windows: Windows, id: WindowId) -> NodeId {
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
