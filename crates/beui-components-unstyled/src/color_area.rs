use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::color::Hsva;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Vec2, pos2};
use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Interactive, Memo, Prop, ReadSignal, Render, clone, component_accessibility,
    component_rect, create_effect, create_memo, create_signal, set_component_state,
};

const STEP: f32 = 0.01;
const PAGE: f32 = 0.1;

pub struct ColorAreaHandle {
    pub color: ReadSignal<Hsva>,
    pub x: Memo<f32>,
    pub y: Memo<f32>,
    pub dragging: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn ColorArea(
    value: Prop<Hsva>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<ColorAreaHandle>,
    on_change: Callback<Hsva>,
    on_drag_change: Callback<bool>,
) -> NodeId {
    let (color, set_color) = create_signal(value.peek());
    create_effect(clone!(set_color -> move || set_color.set(value.get())));
    let (dragging, set_dragging) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let (has_focus, set_has_focus) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let x = create_memo(clone!(color -> move || color.get().saturation));
    let y = create_memo(clone!(color -> move || 1.0 - color.get().value));
    set_component_state(color.clone());

    component_accessibility(create_memo(clone!(color disabled -> move || {
        let color = color.get();
        let mut node = Node::new(Role::Slider);
        let label = label.get();
        node.set_label(if label.is_empty() { "Saturation and brightness".to_owned() } else { label });
        node.set_numeric_value(f64::from((color.saturation * 100.0).round()));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(100.0);
        node.set_numeric_value_step(1.0);
        node.set_value(format!(
            "saturation {}%, brightness {}%",
            (color.saturation * 100.0).round(),
            (color.value * 100.0).round()
        ));
        if disabled.get() {
            node.set_disabled();
        }
        node
    })));

    let set = {
        let color = color.clone();
        let disabled = disabled.clone();
        move |saturation: f32, value: f32| {
            if disabled.get_untracked() {
                return;
            }
            let current = color.get_untracked();
            let next = Hsva::new(current.hue, saturation, value, current.alpha);
            if next != current {
                set_color.set(next);
                on_change.call(next);
            }
        }
    };
    let (drag_set, key_set, step_set) = (set.clone(), set.clone(), set);
    let placed = component_rect();
    let grab = Rc::new(Cell::new(Vec2::ZERO));
    let (press_grab, press_rect, press_color) = (grab.clone(), placed.clone(), color.clone());
    let grabbed = move |press: PointerPress| {
        let rect = press_rect.get_untracked();
        let color = press_color.get_untracked();
        let centre = pos2(
            rect.left() + color.saturation * rect.width(),
            rect.top() + (1.0 - color.value) * rect.height(),
        );
        let offset = centre - press.pos;
        let on_thumb = thumb > 0.0 && offset.length() <= thumb / 2.0;
        press_grab.set(if on_thumb { offset } else { Vec2::ZERO });
    };
    let drag_rect = placed.clone();
    let dragged_to = move |press: PointerPress| {
        let rect = drag_rect.get_untracked();
        let target = press.pos + grab.get();
        match rect.width() > 0.0 && rect.height() > 0.0 {
            true => (
                ((target.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                ((target.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
            ),
            false => (press.fraction.x, press.fraction.y),
        }
    };
    let (pointer, set_pointer) = create_signal(None::<Pos2>);
    let cursor = create_memo(clone!(color placed dragging hovered -> move || {
        if dragging.get() {
            return CursorIcon::Grabbing;
        }
        let over = hovered.get()
            && pointer.get().is_some_and(|pos| {
                let rect = placed.get();
                let color = color.get();
                let centre = pos2(
                    rect.left() + color.saturation * rect.width(),
                    rect.top() + (1.0 - color.value) * rect.height(),
                );
                thumb > 0.0 && (centre - pos).length() <= thumb / 2.0
            });
        match over {
            true => CursorIcon::Grab,
            false => CursorIcon::Crosshair,
        }
    }));
    let (key_color, step_color) = (color.clone(), color.clone());
    let content_node = content.call(ColorAreaHandle {
        color,
        x,
        y,
        dragging: dragging.clone(),
        hovered,
        focused: has_focus,
        disabled: disabled.clone(),
    });
    let tab_stop = create_memo(clone!(disabled -> move || !disabled.get()));
    view! {
        <Interactive
            focusable=true
            tab_stop
            focused
            on_focus_change={move |focused: bool| set_has_focus.set(focused)}
            on_step={move |delta: f32| {
                let color = step_color.get_untracked();
                step_set(color.saturation + delta * STEP, color.value);
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let color = key_color.get_untracked();
                let step = if press.modifiers.shift { PAGE } else { STEP };
                let (saturation, value) = (color.saturation, color.value);
                let next = match press.key {
                    Key::ArrowLeft => (saturation - step, value),
                    Key::ArrowRight => (saturation + step, value),
                    Key::ArrowUp => (saturation, value + step),
                    Key::ArrowDown => (saturation, value - step),
                    Key::PageUp => (saturation, value + PAGE),
                    Key::PageDown => (saturation, value - PAGE),
                    Key::Home => (0.0, value),
                    Key::End => (1.0, value),
                    _ => return false,
                };
                if press.pressed {
                    key_set(next.0, next.1);
                }
                true
            }}
            cursor
            touch_drags=true
            on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
            on_hover_move={move |press: PointerPress| set_pointer.set(Some(press.pos))}
            on_press={grabbed}
            on_drag={move |press: PointerPress| {
                let (x, y) = dragged_to(press);
                drag_set(x, 1.0 - y)
            }}
            on_active_change={move |active: bool| {
                set_dragging.set(active);
                on_drag_change.call(active);
            }}
            children={content_node}
        />
    }
}

pub fn color_area_value(document: &Document, area: NodeId) -> Hsva {
    document
        .component_state::<ReadSignal<Hsva>>(area)
        .get_untracked()
}
