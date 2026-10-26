use crate::frame::Radius;
use beui_macros::{component, view};

use crate::color_picker::{ColorPicker, ColorSwatch};
use crate::focus_ring::FocusRing;
use crate::list::List;
use crate::popover::popover_surface;
use crate::text_input::TextInput;
use crate::tokens::{Size, Space, Stroke};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, ColorInputFieldHandle, ColorInputHandle, ColorInputPickerHandle,
    ColorInputSwatchHandle,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::ItemSize;
use beui_view::types::{Align, Direction};

const SWATCH_WIDTH: Size = Size::Lg;
const SWATCH_HEIGHT: Size = Size::Md;
const SPACING: Space = Space::Md;

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
            surface={popover_surface(None)}
            picker={move |handle: ColorInputPickerHandle| view! {
                <ColorPicker
                    value={handle.value}
                    alpha={handle.alpha}
                    focused={handle.shown}
                    on_preview={handle.on_preview}
                />
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
            <FocusRing focused radius=Radius::Md offset=Stroke::Focus>
                <ColorSwatch color width=SWATCH_WIDTH height=SWATCH_HEIGHT />
            </FocusRing>
        </Tooltip>
    }
}
