use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Callback, Prop, clone, create_effect, create_memo, create_signal};
use crate::styled::text_input::TextInput;

#[component]
pub fn NumberInput(
    value: Prop<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<f64>,
) -> NodeId {
    let (text, set_text) = create_signal(format_number(value.peek()));
    let shown = create_memo(clone!(text -> move || text.get()));
    create_effect(clone!(text set_text -> move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse(&held) != Some(next) {
            set_text.set(format_number(next));
        }
    }));
    let edited = move |typed: String| {
        set_text.set(typed.clone());
        if let Some(parsed) = parse(&typed) {
            on_change.call(parsed.clamp(min, max));
        }
    };
    view! {
        <TextInput
            value={shown}
            label={label}
            placeholder={placeholder}
            disabled={disabled}
            on_change={edited}
        />
    }
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
