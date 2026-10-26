use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::reactive::*;

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
    pub open: Binding<bool>,
    pub on_preview: Callback<Option<Color32>>,
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
    on_preview: Callback<Option<Color32>>,
    swatch: Render<ColorInputSwatchHandle>,
    picker: Render<ColorInputPickerHandle>,
    field: Render<ColorInputFieldHandle>,
    #[prop(children)] content: Render<ColorInputHandle>,
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
    let preview = Callback::new(move |preview: Option<Color32>| {
        previewed.set(preview);
        on_preview.call(preview);
    });
    let swatch_node = view! {
        <Popover
            label={picker_label}
            icon_only=true
            disabled={disabled}
            trigger={move |trigger: PopupTriggerHandle| swatch.call(ColorInputSwatchHandle {
                trigger,
                color: shown,
            })}
        >|popover: PopoverHandle| {
            picker.call(ColorInputPickerHandle {
                value,
                alpha,
                open: popover.open,
                on_preview: preview,
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
