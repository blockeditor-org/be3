use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_view::reactive::Painter;
use beui_view::primitives::*;
use beui_view::reactive::Draw;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::PointerPress;
use beui_view::types::{Direction, ScrollPosition};
use beui_view::types::{Rect, pos2};

const MINIMUM_THUMB: f32 = 0.08;

pub struct ScrollbarHandle {
    pub position: Memo<ScrollPosition>,
    pub direction: Prop<Direction>,
    pub hovered: ReadSignal<bool>,
    pub dragging: ReadSignal<bool>,
    pub engaged: Memo<bool>,
}

pub fn thumb_length(position: ScrollPosition) -> f32 {
    let length = resting_length(position);
    if position.overscroll() == 0.0 || position.viewport <= 0.0 {
        return length;
    }
    let squish = position.viewport / (position.viewport + position.overscroll().abs());
    (length * squish).max(MINIMUM_THUMB.min(length) * 0.5)
}

pub fn thumb_start(position: ScrollPosition) -> f32 {
    if position.overscroll() > 0.0 {
        return 1.0 - thumb_length(position);
    }
    if position.overscroll() < 0.0 {
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
            if color.alpha() == 0 {
                return;
            }
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
        position.clamped() / position.max_offset()
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
    direction.get_untracked().main(press.fraction)
}

#[component]
pub fn Scrollbar(
    position: Prop<ScrollPosition>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = true)] shown: Prop<bool>,
    on_scroll_to: Callback<f32>,
    on_engage: Callback<bool>,
    #[prop(children)] content: Render<ScrollbarHandle>,
) -> NodeId {
    let position = create_memo(move || position.get());
    let hovered = create_signal(false);
    let over_bar = create_signal(false);
    let dragging = create_signal(false);
    let held = create_signal(false);
    let engaged = create_memo(move || over_bar.get() || held.get());
    create_effect(move || on_engage.call(engaged.get()));
    let grab = Rc::new(Cell::new(None::<f32>));

    let content_node = content.call(ScrollbarHandle {
        position,
        direction,
        hovered: hovered.read_only(),
        dragging: dragging.read_only(),
        engaged,
    });

    let (pressed_grab, dragged_grab, active_grab) = (grab.clone(), grab.clone(), grab);
    view! {
        <Interactive
            on_press={move |press: PointerPress| {
                let position = position.get_untracked();
                if position.max_offset() <= 0.0 {
                    return;
                }
                let at = along(&direction, press);
                if over_thumb(position, at) {
                    pressed_grab.set(Some(at - thumb_start(position)));
                    return;
                }
                pressed_grab.set(None);
                let page = match at < thumb_start(position) {
                    true => -position.viewport,
                    false => position.viewport,
                };
                on_scroll_to.call((position.offset + page).clamp(0.0, position.max_offset()));
            }}
            on_drag={move |press: PointerPress| {
                let Some(grab) = dragged_grab.get() else {
                    return;
                };
                let position = position.get_untracked();
                if position.max_offset() <= 0.0 {
                    return;
                }
                on_scroll_to.call(offset_at(position, along(&direction, press) - grab));
            }}
            on_hover_move={move |press: PointerPress| {
                let position = position.get_untracked();
                let at = along(&direction, press);
                hovered.set(position.max_offset() > 0.0 && over_thumb(position, at));
            }}
            on_hover_change={move |inside: bool| {
                over_bar.set(inside);
                if !inside {
                    hovered.set(false);
                }
            }}
            hit_area={create_memo(move || shown.get() && position.get().max_offset() > 0.0)}
            on_active_change={move |active: bool| {
                held.set(active);
                dragging.set(active && active_grab.get().is_some());
                if !active {
                    active_grab.set(None);
                }
            }}
            children={content_node}
        />
    }
}
