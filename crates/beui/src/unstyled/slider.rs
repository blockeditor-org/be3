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
const PAGE_STEPS: f32 = 4.0;
const HALF: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SliderScale {
    #[default]
    Linear,
    Midpoint(f32),
}

impl SliderScale {
    pub fn value_at(self, fraction: f32, min: f32, max: f32) -> f32 {
        let fraction = curved(fraction.clamp(0.0, 1.0), self.exponent(min, max));
        (min + (max - min) * fraction).clamp(min.min(max), max.max(min))
    }

    pub fn fraction_of(self, value: f32, min: f32, max: f32) -> f32 {
        let span = (max - min).max(f32::MIN_POSITIVE);
        let fraction = ((value - min) / span).clamp(0.0, 1.0);
        curved(fraction, self.exponent(min, max).recip())
    }

    fn exponent(self, min: f32, max: f32) -> f32 {
        let Self::Midpoint(midpoint) = self else {
            return 1.0;
        };
        let span = (max - min).max(f32::MIN_POSITIVE);
        let fraction = (midpoint - min) / span;
        if fraction <= 0.0 || fraction >= 1.0 {
            return 1.0;
        }
        let exponent = fraction.ln() / HALF.ln();
        if exponent.is_finite() && exponent > 0.0 {
            exponent
        } else {
            1.0
        }
    }
}

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
    #[prop(default = SliderScale::Linear)] scale: SliderScale,
    #[prop(children)] content: Option<Render<SliderHandle>>,
    on_change: Callback<f32>,
    on_drag_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let value = value.map(move |value| value.clamp(min, max));
    let (value_read, set_value_signal) = create_signal(value.peek());
    create_effect(clone!(set_value_signal -> move || set_value_signal.set(value.get())));
    let fraction =
        create_memo(clone!(value_read -> move || scale.fraction_of(value_read.get(), min, max)));
    let (dragging, set_dragging) = create_signal(false);
    let (focused, set_focused) = create_signal(false);

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Slider)));
    component_accessibility(create_memo(clone!(value_read -> move || {
        let mut node = accessibility.get();
        let value = value_read.get();
        node.set_numeric_value(value.into());
        node.set_min_numeric_value(min.into());
        node.set_max_numeric_value(max.into());
        node.set_numeric_value_step(step_size(scale, value, min, max).into());
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
                let value = untrack(|| value_for_keys.get());
                step_value(stepped(scale, value, delta * STEP, min, max));
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let value = untrack(|| value_read.get());
                let next = match press.key {
                    Key::Home => min,
                    Key::End => max,
                    Key::PageDown => stepped(scale, value, -STEP * PAGE_STEPS, min, max),
                    Key::PageUp => stepped(scale, value, STEP * PAGE_STEPS, min, max),
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
                on_drag={move |press: PointerPress| {
                    set_value(scale.value_at(press.fraction.x, min, max))
                }}
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

fn curved(fraction: f32, exponent: f32) -> f32 {
    if exponent == 1.0 {
        fraction
    } else {
        fraction.powf(exponent)
    }
}

fn stepped(scale: SliderScale, value: f32, delta: f32, min: f32, max: f32) -> f32 {
    scale.value_at(scale.fraction_of(value, min, max) + delta, min, max)
}

fn step_size(scale: SliderScale, value: f32, min: f32, max: f32) -> f32 {
    let up = stepped(scale, value, STEP, min, max) - value;
    let down = value - stepped(scale, value, -STEP, min, max);
    up.max(down).max(0.0)
}

#[cfg(test)]
mod tests;
