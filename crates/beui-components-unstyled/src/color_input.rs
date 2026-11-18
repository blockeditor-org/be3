use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;

use crate::anchored::AnchoredStyle;
use crate::color_picker::HexText;
use crate::popover::{Popover, PopoverHandle};
use crate::popup_trigger::PopupTriggerHandle;

pub struct ColorInputHandle {
    pub swatch: NodeId,
    pub field: NodeId,
}

#[derive(Clone, Copy)]
pub struct ColorInputSwatchHandle {
    pub trigger: PopupTriggerHandle,
    pub color: Memo<Color32>,
}

#[derive(Clone, Copy)]
pub struct ColorInputPickerHandle {
    pub value: Binding<Color32>,
    pub alpha: bool,
    pub shown: Memo<bool>,
    pub on_drag_change: Callback<bool>,
}

#[derive(Clone, Copy)]
pub struct ColorInputFieldHandle {
    pub hex: HexText,
    pub label: Memo<String>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn ColorInput(
    value: Binding<Color32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] alpha: bool,
    on_drag_change: Callback<bool>,
    swatch: Render<ColorInputSwatchHandle, NodeId>,
    surface: AnchoredStyle,
    picker: RenderFn<ColorInputPickerHandle, NodeId>,
    field: Render<ColorInputFieldHandle, NodeId>,
    #[prop(children)] content: Render<ColorInputHandle, NodeId>,
) -> NodeId {
    let shown = create_memo(move || value.get());
    let hex = HexText::new(shown, alpha, Callback::from(value));
    let label = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    let swatch_node = view! {
        <Popover
            label={move || match label.get() {
                label if label.is_empty() => "Choose a color".to_owned(),
                label => format!("Choose {label}"),
            }}
            icon_only=true
            disabled={disabled}
            surface
            trigger={move |trigger: PopupTriggerHandle| swatch.call(ColorInputSwatchHandle {
                trigger,
                color: shown,
            })}
        >|popover: PopoverHandle| {
            picker.call(ColorInputPickerHandle {
                value,
                alpha,
                shown: popover.shown,
                on_drag_change,
            })
        }</Popover>
    };
    let field_node = field.call(ColorInputFieldHandle {
        hex,
        label,
        disabled,
    });
    content.call(ColorInputHandle {
        swatch: swatch_node,
        field: field_node,
    })
}
