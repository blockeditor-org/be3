use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};

use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const STEP: f32 = 0.05;
const PAGE_STEPS: f32 = 4.0;
const STRAIGHT: f32 = 1.0;
const CURVE_TOLERANCE: f32 = 0.001;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SliderScale {
    #[default]
    Linear,
    Midpoint(f32),
}

impl SliderScale {
    pub fn value_at(self, fraction: f32, min: f32, max: f32) -> f32 {
        let fraction = curved(fraction.clamp(0.0, 1.0), self.base(min, max));
        (min + (max - min) * fraction).clamp(min.min(max), max.max(min))
    }

    pub fn fraction_of(self, value: f32, min: f32, max: f32) -> f32 {
        let span = (max - min).max(f32::MIN_POSITIVE);
        let fraction = ((value - min) / span).clamp(0.0, 1.0);
        straightened(fraction, self.base(min, max))
    }

    fn base(self, min: f32, max: f32) -> f32 {
        let Self::Midpoint(midpoint) = self else {
            return STRAIGHT;
        };
        let below = midpoint - min;
        let above = max - midpoint;
        if below <= 0.0 || above <= 0.0 {
            return STRAIGHT;
        }
        let base = (above / below).powi(2);
        if base.is_finite() && base > 0.0 && (base - STRAIGHT).abs() > CURVE_TOLERANCE {
            base
        } else {
            STRAIGHT
        }
    }
}

pub struct SliderHandle {
    pub value: Binding<f32>,
    pub fraction: Memo<f32>,
    pub dragging: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn Slider(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SliderApi>,
    value: Binding<f32>,
    #[prop(default = 0.0)] min: f32,
    #[prop(default = 1.0)] max: f32,
    #[prop(default = SliderScale::Linear)] scale: SliderScale,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(children)] content: Option<Render<SliderHandle>>,
    on_drag_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let current = create_memo(move || value.get().clamp(min, max));
    let fraction = create_memo(move || scale.fraction_of(current.get(), min, max));
    let dragging = create_signal(false);
    let focused = create_signal(false);
    let disabled = create_memo(move || disabled.get());

    let accessibility = crate::labelled_node(Role::Slider, accessibility, label);
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        let value = current.get();
        node.set_numeric_value(value.into());
        node.set_min_numeric_value(min.into());
        node.set_max_numeric_value(max.into());
        node.set_numeric_value_step(step_size(scale, value, min, max).into());
        if disabled.get() {
            node.set_disabled();
        } else {
            node.clear_disabled();
        }
        node
    }));

    let content_node = content.map(|build| {
        build.call(SliderHandle {
            value,
            fraction,
            dragging: dragging.read_only(),
            focused: focused.read_only(),
            disabled,
        })
    });

    let asked = StoredCell::new(None::<(f32, f32)>);
    let set_value = move |next: f32| {
        if disabled.get_untracked() {
            return;
        }
        let next = next.clamp(min, max);
        let shown = current.get_untracked();
        if shown == next || asked.get() == Some((shown, next)) {
            return;
        }
        asked.set(Some((shown, next)));
        value.set(next);
    };
    let grab = Rc::new(Cell::new(0.0f32));
    let press_grab = grab.clone();
    let dragged_to = move |press: PointerPress| {
        let travel = press.size.x - thumb;
        match travel > 0.0 {
            true => (press.local.x + grab.get() - thumb / 2.0) / travel,
            false => press.fraction.x,
        }
    };
    let jump_to = dragged_to.clone();
    let grabbed = move |press: PointerPress| {
        let travel = (press.size.x - thumb).max(0.0);
        let centre = thumb / 2.0 + fraction.get_untracked() * travel;
        let on_thumb = thumb > 0.0 && (press.local.x - centre).abs() <= thumb / 2.0;
        press_grab.set(if on_thumb {
            centre - press.local.x
        } else {
            0.0
        });
        if !on_thumb {
            set_value(scale.value_at(jump_to(press), min, max));
        }
    };

    api.fill(SliderApi(value));

    let tab_stop = create_memo(move || !disabled.get());
    view! {
        <Interactive
            focusable=true
            tab_stop={tab_stop}
            on_focus_change={move |next: bool| {
                focused.set(next);
                on_focus_change.call(next);
            }}
            on_step={move |delta: f32| {
                let value = current.get_untracked();
                set_value(stepped(scale, value, delta * STEP, min, max));
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let value = current.get_untracked();
                let next = match press.key {
                    Key::Home => min,
                    Key::End => max,
                    Key::PageDown => stepped(scale, value, -STEP * PAGE_STEPS, min, max),
                    Key::PageUp => stepped(scale, value, STEP * PAGE_STEPS, min, max),
                    _ => return false,
                };
                if press.pressed {
                    set_value(next);
                }
                true
            }}
            cursor=CursorIcon::PointingHand
            on_press={grabbed}
            on_drag={move |press: PointerPress| {
                set_value(scale.value_at(dragged_to(press), min, max))
            }}
            on_active_change={move |next: bool| {
                dragging.set(next);
                on_drag_change.call(next);
            }}
            children={content_node}
        />
    }
}

fn curved(fraction: f32, base: f32) -> f32 {
    if base == STRAIGHT {
        return fraction;
    }
    ((base.powf(fraction) - 1.0) / (base - 1.0)).clamp(0.0, 1.0)
}

fn straightened(fraction: f32, base: f32) -> f32 {
    if base == STRAIGHT {
        return fraction;
    }
    (fraction.mul_add(base - 1.0, 1.0).ln() / base.ln()).clamp(0.0, 1.0)
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

#[derive(Clone, Copy)]
pub struct SliderApi(Binding<f32>);

impl SliderApi {
    pub fn value(&self) -> Binding<f32> {
        self.0
    }
}
