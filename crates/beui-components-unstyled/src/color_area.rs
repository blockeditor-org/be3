use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Hsva;
use beui_view::types::{CursorIcon, Key, KeyPress, PointerPress};
use beui_view::types::{Pos2, Vec2, pos2};

const STEP: f32 = 0.01;
const PAGE: f32 = 0.1;

pub struct ColorAreaHandle {
    pub color: Binding<Hsva>,
    pub x: Memo<f32>,
    pub y: Memo<f32>,
    pub dragging: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn ColorArea(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ColorAreaApi>,
    value: Binding<Hsva>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<ColorAreaHandle>,
    on_drag_change: Callback<bool>,
) -> NodeId {
    let color = value;
    let dragging = create_signal(false);
    let hovered = create_signal(false);
    let has_focus = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let x = create_memo(move || color.get().saturation);
    let y = create_memo(move || 1.0 - color.get().value);
    api.fill(ColorAreaApi(color));

    component_accessibility(create_memo(move || {
        let color = color.get();
        let mut node = Node::new(Role::Slider);
        let label = label.get();
        node.set_label(if label.is_empty() {
            "Saturation and brightness".to_owned()
        } else {
            label
        });
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
    }));

    let set = move |saturation: f32, value: f32| {
        if disabled.get_untracked() {
            return;
        }
        let current = color.get_untracked();
        let next = Hsva::new(current.hue, saturation, value, current.alpha);
        if next != current {
            color.set(next);
        }
    };
    let grab = Rc::new(Cell::new(Vec2::ZERO));
    let (press_grab, press_color) = (grab.clone(), color);
    let grabbed = move |press: PointerPress| {
        let color = press_color.get_untracked();
        let offset = thumb_centre(press.size, color) - press.local;
        let on_thumb = thumb > 0.0 && offset.length() <= thumb / 2.0;
        press_grab.set(if on_thumb { offset } else { Vec2::ZERO });
    };
    let dragged_to = move |press: PointerPress| {
        let size = press.size;
        let target = press.local + grab.get();
        match size.x > 0.0 && size.y > 0.0 {
            true => (
                (target.x / size.x).clamp(0.0, 1.0),
                (target.y / size.y).clamp(0.0, 1.0),
            ),
            false => (press.fraction.x, press.fraction.y),
        }
    };
    let pointer = create_signal(None::<PointerPress>);
    let cursor = create_memo(move || {
        if dragging.get() {
            return CursorIcon::Grabbing;
        }
        let over = hovered.get()
            && pointer.get().is_some_and(|press| {
                let centre = thumb_centre(press.size, color.get());
                thumb > 0.0 && (centre - press.local).length() <= thumb / 2.0
            });
        match over {
            true => CursorIcon::Grab,
            false => CursorIcon::Crosshair,
        }
    });
    let content_node = content.call(ColorAreaHandle {
        color,
        x,
        y,
        dragging: dragging.read_only(),
        hovered: hovered.read_only(),
        focused: has_focus.read_only(),
        disabled,
    });
    let tab_stop = create_memo(move || !disabled.get());
    view! {
        <Interactive
            focusable=true
            tab_stop
            focused
            on_focus_change={has_focus}
            on_step={move |delta: f32| {
                let color = color.get_untracked();
                set(color.saturation + delta * STEP, color.value);
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let color = color.get_untracked();
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
                    set(next.0, next.1);
                }
                true
            }}
            cursor
            touch_drags=true
            on_hover_change={hovered}
            on_hover_move={move |press: PointerPress| pointer.set(Some(press))}
            on_press={grabbed}
            on_drag={move |press: PointerPress| {
                let (x, y) = dragged_to(press);
                set(x, 1.0 - y)
            }}
            on_active_change={move |active: bool| {
                dragging.set(active);
                on_drag_change.call(active);
            }}
            children={content_node}
        />
    }
}

fn thumb_centre(size: Vec2, color: Hsva) -> Pos2 {
    pos2(color.saturation * size.x, (1.0 - color.value) * size.y)
}

#[derive(Clone, Copy)]
pub struct ColorAreaApi(Binding<Hsva>);

impl ColorAreaApi {
    pub fn value(&self) -> Hsva {
        self.0.get_untracked()
    }
}
