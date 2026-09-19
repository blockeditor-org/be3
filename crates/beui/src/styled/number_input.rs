use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, NodeRef, Prop, clone, create_effect, create_memo, create_signal,
    set_component_state,
};
use crate::styled::text_input::TextInput;

const DRAG_THRESHOLD: f32 = 2.0;

struct Field(NodeRef);

#[derive(Clone, Copy, PartialEq)]
pub enum NumberDrag {
    Off,
    Linear { speed: f64 },
    Logarithmic { factor: f64 },
}

impl NumberDrag {
    fn applied(self, start: f64, points: f64) -> Option<f64> {
        match self {
            NumberDrag::Off => None,
            NumberDrag::Linear { speed } => Some(start + points * speed),
            NumberDrag::Logarithmic { factor } if factor > 1.0 => {
                let base = factor.ln();
                let exponent = start.max(f64::MIN_POSITIVE).ln() / base;
                Some(factor.powf(exponent + points))
            }
            NumberDrag::Logarithmic { .. } => None,
        }
    }
}

#[component]
pub fn NumberInput(
    value: Prop<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = NumberDrag::Linear { speed: 1.0 })] drag: NumberDrag,
    on_change: Callback<f64>,
) -> NodeId {
    let (text, set_text) = create_signal(format_number(value.peek()));
    let (editing, set_editing) = create_signal(false);
    let (wanted, set_wanted) = create_signal(false);
    create_effect(clone!(value text set_text -> move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse(&held) != Some(next) {
            set_text.set(format_number(next));
        }
    }));
    let off = create_memo(clone!(disabled -> move || disabled.get()));
    let grabbable = create_memo(clone!(off editing -> move || {
        drag != NumberDrag::Off && !off.get() && !editing.get()
    }));
    let cursor = create_memo(clone!(grabbable -> move || match grabbable.get() {
        true => CursorIcon::ResizeHorizontal,
        false => CursorIcon::Text,
    }));

    let held: Rc<Cell<Option<(Pos2, f64, bool)>>> = Rc::default();
    let pressed = clone!(held value -> move |press: PointerPress| {
        held.set(Some((press.pos, value.peek(), false)));
    });
    let changed = on_change.clone();
    let dragged = clone!(held set_text -> move |at: PointerPress| {
        let Some((origin, start, moved)) = held.get() else {
            return;
        };
        let points = at.pos.x - origin.x;
        if !moved && points.abs() < DRAG_THRESHOLD {
            return;
        }
        let Some(next) = drag.applied(start, f64::from(points)) else {
            return;
        };
        held.set(Some((origin, start, true)));
        let next = next.clamp(min, max);
        set_text.set(format_number(next));
        changed.call(next);
    });
    let clicked = clone!(held set_wanted -> move || {
        if let Some((_, _, moved)) = held.take()
            && !moved
        {
            set_wanted.set(true);
        }
    });

    let edited = move |typed: String| {
        set_text.set(typed.clone());
        if let Some(parsed) = parse(&typed) {
            on_change.call(parsed.clamp(min, max));
        }
    };
    let field = NodeRef::new();
    set_component_state(Field(field.clone()));
    let focus_changed = clone!(set_editing set_wanted -> move |focused: bool| {
        set_editing.set(focused);
        if !focused {
            set_wanted.set(false);
        }
    });
    view! {
        <ClickCatcher
            cursor={cursor}
            capture_presses={grabbable}
            on_press={pressed}
            on_drag={dragged}
            on_click={clicked}
        >
            <TextInput
                @node_ref=&field
                value={text}
                label={label}
                placeholder={placeholder}
                disabled={disabled}
                focused={wanted}
                on_change={edited}
                on_focus_change={focus_changed}
            />
        </ClickCatcher>
    }
}

pub fn number_input_field(document: &Document, input: NodeId) -> NodeId {
    document.component_state::<Field>(input).0.get()
}

fn parse(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    match trimmed.is_empty() {
        true => None,
        false => trimmed
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite()),
    }
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}
