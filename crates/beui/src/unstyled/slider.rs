use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Focusable, Memo, Prop, ReadSignal, Render, clone,
    component_accessibility, create_effect, create_memo, create_signal, set_component_state,
    untrack,
};

const STEP: f32 = 0.05;

pub struct SliderHandle {
    pub value: ReadSignal<f32>,
    pub fraction: Memo<f32>,
    pub dragging: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn Slider(
    value: Prop<f32>,
    #[prop(default = 0.0)] min: f32,
    #[prop(default = 1.0)] max: f32,
    #[prop(children)] content: Option<Render<SliderHandle>>,
    on_change: Callback<f32>,
    on_drag_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let span = (max - min).max(f32::MIN_POSITIVE);
    let value = value.map(move |value| value.clamp(min, max));
    let (value_read, set_value_signal) = create_signal(value.peek());
    create_effect(clone!(set_value_signal -> move || set_value_signal.set(value.get())));
    let fraction = create_memo(clone!(value_read -> move || (value_read.get() - min) / span));
    let (dragging, set_dragging) = create_signal(false);
    let (focused, set_focused) = create_signal(false);

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Slider)));
    component_accessibility(create_memo(clone!(value_read -> move || {
        let mut node = accessibility.get();
        node.set_numeric_value(value_read.get().into());
        node.set_min_numeric_value(min.into());
        node.set_max_numeric_value(max.into());
        node.set_numeric_value_step((STEP * span).into());
        node
    })));

    let content_node = content.map(|build| {
        build.call(SliderHandle {
            value: value_read.clone(),
            fraction: fraction.clone(),
            dragging: dragging.clone(),
            focused: focused.clone(),
        })
    });

    let set_value = {
        let value = value_read.clone();
        move |next: f32| {
            let next = next.clamp(min, max);
            if untrack(|| value.get()) == next {
                return;
            }
            set_value_signal.set(next);
            on_change.call(next);
        }
    };
    let step_value = set_value.clone();
    let key_value = set_value.clone();
    let value_for_keys = value_read.clone();

    set_component_state(value_read.clone());

    view! {
        <Focusable
            on_focus_change={move |focused: bool| {
                set_focused.set(focused);
                on_focus_change.call(focused);
            }}
            on_step={move |delta: f32| {
                step_value(untrack(|| value_for_keys.get()) + delta * STEP * span);
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let value = untrack(|| value_read.get());
                let next = match press.key {
                    Key::Home => min,
                    Key::End => max,
                    Key::PageDown => value - STEP * span * 4.0,
                    Key::PageUp => value + STEP * span * 4.0,
                    _ => return false,
                };
                if press.pressed {
                    key_value(next);
                }
                true
            }}
        >
            <ClickCatcher
                cursor=CursorIcon::PointingHand
                on_drag={move |press: PointerPress| set_value(min + press.fraction.x * span)}
                on_active_change={move |dragging: bool| {
                    set_dragging.set(dragging);
                    on_drag_change.call(dragging);
                }}
                children={content_node}
            />
        </Focusable>
    }
}

pub fn slider_value(document: &Document, slider: NodeId) -> ReadSignal<f32> {
    document.component_state::<ReadSignal<f32>>(slider).clone()
}
