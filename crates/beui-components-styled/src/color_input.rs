use crate::frame::Radius;
use beui_macros::{component, view};

use crate::color_picker::{ColorPicker, ColorSwatch};
use crate::focus_ring::FocusRing;
use crate::popover::PopoverPanel;
use crate::text_input::TextInput;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, HexText, PopoverHandle, PopupTriggerHandle};
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
    value: Binding<Color32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] alpha: bool,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let previewed = create_signal(None::<Color32>);
    let shown = create_memo(move || previewed.get().unwrap_or_else(|| value.get()));
    let hex = HexText::new(shown, alpha, value.into_callback());
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
                icon_only=true
                disabled={disabled}
                trigger={move |handle: PopupTriggerHandle| view! {
                    <SwatchTrigger handle color={shown} />
                }}
            >|popover: PopoverHandle|
                <PopoverPanel>
                    <ColorPicker
                        value
                        alpha
                        focused={popover.open}
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
                on_submit={move |typed: String| hex.submit(typed)}
            />
        </List>
    }
}

#[component]
fn SwatchTrigger(handle: PopupTriggerHandle, color: Memo<Color32>) -> NodeId {
    let ButtonHandle {
        focused, tooltip, ..
    } = handle.button;
    view! {
        <Tooltip label={tooltip}>
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
