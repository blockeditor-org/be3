use crate::frame::Radius;
use beui_macros::{component, view};

use crate::color_picker::{ColorPicker, ColorSwatch};
use crate::focus_ring::FocusRing;
use crate::popover::PopoverPanel;
use crate::text_input::TextInput;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, HexText, PopoverHandle, PopoverTriggerHandle};
use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const SWATCH_WIDTH: f32 = 32.0;
const SWATCH_HEIGHT: f32 = 24.0;
const SPACING: f32 = 8.0;
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
    let current = create_signal(value.get_untracked());
    create_effect(move || current.set(value.get()));
    let previewed = create_signal(None::<Color32>);
    let shown = create_memo(move || previewed.get().unwrap_or_else(|| current.get()));
    let report = move |color: Color32| {
        if color != current.get_untracked() {
            current.set(color);
            on_change.call(color);
        }
    };
    let hex = HexText::new(shown, alpha, Callback::new(report));
    let (edit, submit) = (hex, hex);
    let label = create_memo(move || label.get());
    let picker_label = create_memo(move || match label.get() {
        label if label.is_empty() => "Choose a color".to_owned(),
        label => format!("Choose {label}"),
    });
    let disabled = create_memo(move || disabled.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <unstyled::Popover
                label={picker_label}
                disabled={disabled}
                trigger={move |handle: PopoverTriggerHandle| view! {
                    <SwatchTrigger handle color={shown} label={picker_label} />
                }}
            >|popover: PopoverHandle|
                <PopoverPanel>
                    <ColorPicker
                        value={current}
                        alpha
                        focused={popover.open}
                        on_change={report}
                        on_preview={move |preview: Option<Color32>| {
                            previewed.set(preview);
                            on_preview.call(preview);
                        }}
                    />
                </PopoverPanel>
            </unstyled::Popover>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={hex.text()}
                label={label}
                placeholder={hex.placeholder()}
                disabled={disabled}
                on_change={move |typed: String| edit.edit(typed)}
                on_submit={move |typed: String| submit.submit(typed)}
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
    let PopoverTriggerHandle {
        open,
        button: ButtonHandle { focused, .. },
    } = handle;
    view! {
        <Tooltip label disabled={open}>
            <FocusRing
                focused
                radius={Radius::custom(Radius::Md.px() + 2.0)}
                offset=FOCUS_RING_OFFSET
            >
                <ColorSwatch color width=SWATCH_WIDTH height=SWATCH_HEIGHT />
            </FocusRing>
        </Tooltip>
    }
}
