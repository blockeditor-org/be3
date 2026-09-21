use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::base::{Direction, ScrollPosition};
use crate::input::PointerPress;
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Memo, Prop, ReadSignal, Render, clone, create_memo, create_signal,
    untrack,
};

const MINIMUM_THUMB: f32 = 0.08;

pub struct ScrollbarHandle {
    pub position: Memo<ScrollPosition>,
    pub direction: Prop<Direction>,
    pub hovered: ReadSignal<bool>,
    pub dragging: ReadSignal<bool>,
}

pub fn thumb_length(position: ScrollPosition) -> f32 {
    if position.content > 0.0 {
        (position.viewport / position.content).clamp(MINIMUM_THUMB, 1.0)
    } else {
        1.0
    }
}

pub fn thumb_start(position: ScrollPosition) -> f32 {
    thumb_travel(position) * progress(position)
}

fn thumb_travel(position: ScrollPosition) -> f32 {
    1.0 - thumb_length(position)
}

fn progress(position: ScrollPosition) -> f32 {
    if position.max_offset() > 0.0 {
        position.offset / position.max_offset()
    } else {
        0.0
    }
}

fn offset_at(position: ScrollPosition, start: f32) -> f32 {
    let travel = thumb_travel(position);
    if travel <= 0.0 {
        return 0.0;
    }
    (start / travel * position.max_offset()).clamp(0.0, position.max_offset())
}

fn over_thumb(position: ScrollPosition, at: f32) -> bool {
    let start = thumb_start(position);
    at >= start && at <= start + thumb_length(position)
}

fn along(direction: &Prop<Direction>, press: PointerPress) -> f32 {
    untrack(|| direction.get()).main(press.fraction)
}

#[component]
pub fn Scrollbar(
    position: Prop<ScrollPosition>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    on_scroll_to: Callback<f32>,
    #[prop(children)] content: Render<ScrollbarHandle>,
) -> NodeId {
    let position = create_memo(move || position.get());
    let (hovered, set_hovered) = create_signal(false);
    let (dragging, set_dragging) = create_signal(false);
    let grab = Rc::new(Cell::new(None::<f32>));

    let content_node = content.call(ScrollbarHandle {
        position: position.clone(),
        direction: direction.clone(),
        hovered: hovered.clone(),
        dragging: dragging.clone(),
    });

    let paged = on_scroll_to.clone();
    let (pressed_axis, dragged_axis, hovered_axis) =
        (direction.clone(), direction.clone(), direction.clone());
    let (pressed_at, dragged_at, hovered_at) = (position.clone(), position.clone(), position);
    let (pressed_grab, dragged_grab, active_grab) = (grab.clone(), grab.clone(), grab);
    view! {
        <ClickCatcher
            on_press={move |press: PointerPress| {
                let position = untrack(|| pressed_at.get());
                if position.max_offset() <= 0.0 {
                    return;
                }
                let at = along(&pressed_axis, press);
                if over_thumb(position, at) {
                    pressed_grab.set(Some(at - thumb_start(position)));
                    return;
                }
                pressed_grab.set(None);
                let page = match at < thumb_start(position) {
                    true => -position.viewport,
                    false => position.viewport,
                };
                paged.call((position.offset + page).clamp(0.0, position.max_offset()));
            }}
            on_drag={move |press: PointerPress| {
                let Some(grab) = dragged_grab.get() else {
                    return;
                };
                let position = untrack(|| dragged_at.get());
                if position.max_offset() <= 0.0 {
                    return;
                }
                on_scroll_to.call(offset_at(position, along(&dragged_axis, press) - grab));
            }}
            on_hover_move={clone!(set_hovered -> move |press: PointerPress| {
                let position = untrack(|| hovered_at.get());
                let at = along(&hovered_axis, press);
                set_hovered.set(position.max_offset() > 0.0 && over_thumb(position, at));
            })}
            on_hover_change={move |inside: bool| {
                if !inside {
                    set_hovered.set(false);
                }
            }}
            on_active_change={move |active: bool| {
                set_dragging.set(active && active_grab.get().is_some());
                if !active {
                    active_grab.set(None);
                }
            }}
            children={content_node}
        />
    }
}
