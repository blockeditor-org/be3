use crate::frame::Radius;
use beui_macros::{component, view};

use crate::color_picker::{ColorPicker, ColorSwatch};
use crate::focus_ring::FocusRing;
use crate::popover::PopoverPanel;
use crate::text_input::TextInput;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, ColorInputFieldHandle, ColorInputHandle, ColorInputPickerHandle,
    ColorInputSwatchHandle,
};
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
    view! {
        <unstyled::ColorInput
            value
            label
            disabled
            alpha
            on_preview
            swatch={move |handle: ColorInputSwatchHandle| view! {
                <SwatchTrigger handle />
            }}
            picker={move |handle: ColorInputPickerHandle| view! {
                <PopoverPanel>
                    <ColorPicker
                        value={handle.value}
                        alpha={handle.alpha}
                        focused={handle.open}
                        on_preview={handle.on_preview}
                    />
                </PopoverPanel>
            }}
            field={move |handle: ColorInputFieldHandle| {
                let ColorInputFieldHandle { hex, label, disabled } = handle;
                view! {
                    <TextInput
                        value={hex.text()}
                        label={label}
                        placeholder={hex.placeholder()}
                        disabled={disabled}
                        on_submit={move |typed: String| hex.submit(typed)}
                    />
                }
            }}
        >|handle: ColorInputHandle| {
            let ColorInputHandle { swatch, field } = handle;
            view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    {swatch}
                    {field} @sizing=ItemSize::Percent(100.0)
                </List>
            }
        }</unstyled::ColorInput>
    }
}

#[component]
fn SwatchTrigger(handle: ColorInputSwatchHandle) -> NodeId {
    let ColorInputSwatchHandle { trigger, color } = handle;
    let ButtonHandle {
        focused, tooltip, ..
    } = trigger.button;
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
