use beui_macros::{component, view};

use crate::base::{Align, Direction};
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{
    Callback, Frame, ItemSize, List, Prop, clone, create_effect, create_signal,
};
use crate::styled::text_input::TextInput;
use crate::styled::theme::{RADIUS, use_theme};

const SWATCH_WIDTH: f32 = 32.0;
const SWATCH_HEIGHT: f32 = 24.0;
const SPACING: f32 = 8.0;

#[component]
pub fn ColorInput(
    value: Prop<Color32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Color32>,
) -> NodeId {
    let (text, set_text) = create_signal(format_color(value.peek()));
    create_effect(clone!(value text set_text -> move || {
        let next = value.get();
        let held = text.get_untracked();
        if parse_color(&held) != Some(next) {
            set_text.set(format_color(next));
        }
    }));
    let edited = move |typed: String| {
        set_text.set(typed.clone());
        if let Some(parsed) = parse_color(&typed) {
            on_change.call(parsed);
        }
    };
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Frame
                @sizing=ItemSize::Fixed(SWATCH_WIDTH)
                height=SWATCH_HEIGHT
                color={value}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=RADIUS
            />
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={text}
                label={label}
                placeholder="#RRGGBBAA"
                disabled={disabled}
                on_change={edited}
            />
        </List>
    }
}

pub fn format_color(color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_array();
    format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}")
}

pub fn parse_color(text: &str) -> Option<Color32> {
    let text = text.trim().strip_prefix('#')?;
    if !text.is_ascii() || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| u8::from_str_radix(&text[start..start + 2], 16).ok();
    match text.len() {
        6 => Some(Color32::from_rgba_unmultiplied(
            channel(0)?,
            channel(2)?,
            channel(4)?,
            255,
        )),
        8 => Some(Color32::from_rgba_unmultiplied(
            channel(0)?,
            channel(2)?,
            channel(4)?,
            channel(6)?,
        )),
        _ => None,
    }
}
