use beui_macros::{component, view};

use crate::base::{Align, Direction};
use crate::color::{Color32, format_hex};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Frame, ItemSize, List, Memo, Prop, clone, create_effect, create_memo, create_signal,
    focus_ring,
};
use crate::styled::color_picker::{ColorPicker, ColorSwatch};
use crate::styled::popover::PopoverPanel;
use crate::styled::text_input::TextInput;
use crate::styled::theme::{RADIUS, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{PopoverHandle, PopoverTriggerHandle};

const SWATCH_WIDTH: f32 = 32.0;
const SWATCH_HEIGHT: f32 = 24.0;
const SPACING: f32 = 8.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 2.0;

#[component]
pub fn ColorInput(
    value: Prop<Color32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] alpha: bool,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let (current, set_current) = create_signal(value.peek());
    create_effect(clone!(set_current -> move || set_current.set(value.get())));
    let (previewed, set_previewed) = create_signal(None::<Color32>);
    let shown = create_memo(clone!(current previewed -> move || {
        previewed.get().unwrap_or_else(|| current.get())
    }));
    let (text, set_text) = create_signal(format_color_for(current.get_untracked(), alpha));
    create_effect(clone!(shown text set_text -> move || {
        let next = shown.get();
        let held = text.get_untracked();
        if parse_color(&held) != Some(next) {
            set_text.set(format_color_for(next, alpha));
        }
    }));
    let report = clone!(current set_current -> move |color: Color32| {
        if color != current.get_untracked() {
            set_current.set(color);
            on_change.call(color);
        }
    });
    let typed_report = report.clone();
    let edited = move |typed: String| {
        set_text.set(typed.clone());
        if let Some(parsed) = parse_color(&typed) {
            typed_report(parsed);
        }
    };
    let label = create_memo(move || label.get());
    let picker_label = create_memo(clone!(label -> move || match label.get() {
        label if label.is_empty() => "Choose a color".to_owned(),
        label => format!("Choose {label}"),
    }));
    let disabled = create_memo(move || disabled.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <unstyled::Popover
                label={picker_label.clone()}
                disabled={disabled.clone()}
                trigger={move |handle: PopoverTriggerHandle| view! {
                    <SwatchTrigger handle color={shown.clone()} label={picker_label} />
                }}
            >
                {move |popover: PopoverHandle| view! {
                    <PopoverPanel>
                        <ColorPicker
                            value={current}
                            alpha
                            focused={popover.open}
                            on_change={report}
                            on_preview={move |preview: Option<Color32>| {
                                set_previewed.set(preview);
                                on_preview.call(preview);
                            }}
                        />
                    </PopoverPanel>
                }}
            </unstyled::Popover>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={text}
                label={label}
                placeholder={if alpha { "#RRGGBBAA" } else { "#RRGGBB" }}
                disabled={disabled}
                on_change={edited}
            />
        </List>
    }
}

#[component]
fn SwatchTrigger(
    handle: PopoverTriggerHandle,
    color: Memo<Color32>,
    label: Memo<String>,
) -> NodeId {
    let PopoverTriggerHandle { open, focused, .. } = handle;
    let theme = use_theme();
    view! {
        <Tooltip label disabled={open}>
            <Frame
                outline={theme.accent.clone()}
                outline_width=FOCUS_RING_WIDTH
                radius={RADIUS + 2}
                outline_offset=FOCUS_RING_OFFSET
                outline_visible={focus_ring(focused)}
            >
                <ColorSwatch color width=SWATCH_WIDTH height=SWATCH_HEIGHT />
            </Frame>
        </Tooltip>
    }
}

fn format_color_for(color: Color32, alpha: bool) -> String {
    match alpha {
        true => format_color(color),
        false => format_hex(color, false),
    }
}

pub fn format_color(color: Color32) -> String {
    format_hex(color, true)
}

pub fn parse_color(text: &str) -> Option<Color32> {
    let digits = text.trim().strip_prefix('#')?;
    match digits.len() {
        6 | 8 => crate::color::parse_hex(digits),
        _ => None,
    }
}
