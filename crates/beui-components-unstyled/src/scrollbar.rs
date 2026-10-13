use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::drawing::Draw;
use beui_core::base::{Direction, ScrollPosition};
use beui_core::color::Color32;
use beui_core::geometry::{Rect, pos2};
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_view::reactive::{
    Callback, Drawing, Interactive, Memo, Prop, ReadSignal, Render, create_memo, create_signal,
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
    let length = resting_length(position);
    if position.overscroll == 0.0 || position.viewport <= 0.0 {
        return length;
    }
    let squish = position.viewport / (position.viewport + position.overscroll.abs());
    (length * squish).max(MINIMUM_THUMB.min(length) * 0.5)
}

pub fn thumb_start(position: ScrollPosition) -> f32 {
    if position.overscroll > 0.0 {
        return 1.0 - thumb_length(position);
    }
    if position.overscroll < 0.0 {
        return 0.0;
    }
    thumb_travel(position) * progress(position)
}

pub fn thumb_rect(rect: Rect, direction: Direction, position: ScrollPosition) -> Rect {
    let length = direction.main(rect.size());
    let start = direction.main(rect.min.to_vec2()) + thumb_start(position) * length;
    let end = start + thumb_length(position) * length;
    match direction {
        Direction::Horizontal => {
            Rect::from_min_max(pos2(start, rect.top()), pos2(end, rect.bottom()))
        }
        Direction::Vertical => {
            Rect::from_min_max(pos2(rect.left(), start), pos2(rect.right(), end))
        }
    }
}

#[component]
pub fn ScrollbarThumb(
    position: Memo<ScrollPosition>,
    direction: Prop<Direction>,
    color: Prop<Color32>,
    #[prop(default = 0.0)] radius: f32,
) -> NodeId {
    let draw = Prop::dynamic(move || {
        let (position, direction, color) = (position.get(), direction.get(), color.get());
        Rc::new(move |painter: &Painter, rect: Rect| {
            let thumb = painter
                .pixel_grid()
                .snap_rect(thumb_rect(rect, direction, position));
            painter.rect_filled(thumb, radius, color);
        }) as Draw
    });
    view! {
        <Drawing draw />
    }
}

fn resting_length(position: ScrollPosition) -> f32 {
    if position.content > 0.0 {
        (position.viewport / position.content).clamp(MINIMUM_THUMB, 1.0)
    } else {
        1.0
    }
}

fn thumb_travel(position: ScrollPosition) -> f32 {
    1.0 - resting_length(position)
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
        position,
        direction: direction,
        hovered,
        dragging,
    });

    let paged = on_scroll_to;
    let (pressed_axis, dragged_axis, hovered_axis) = (direction, direction, direction);
    let (pressed_at, dragged_at, hovered_at) = (position, position, position);
    let (pressed_grab, dragged_grab, active_grab) = (grab.clone(), grab.clone(), grab);
    view! {
        <Interactive
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
            on_hover_move={move |press: PointerPress| {
                let position = untrack(|| hovered_at.get());
                let at = along(&hovered_axis, press);
                set_hovered.set(position.max_offset() > 0.0 && over_thumb(position, at));
            }}
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
