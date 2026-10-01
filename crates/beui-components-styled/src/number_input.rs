use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::text_input::TextInput;
use crate::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{NumberDrag, NumberFaceHandle, NumberFieldHandle};
use beui_core::base::TextAlign;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Frame, NodeRef, Prop, Text, clone, create_memo, focus_ring, set_component_state,
};

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;

struct FaceText(NodeRef);

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
    on_preview: Callback<Option<f64>>,
) -> NodeId {
    let shown = NodeRef::new();
    set_component_state(FaceText(shown.clone()));
    let (field_label, field_disabled) = (label.clone(), disabled.clone());
    let face_placeholder = placeholder.clone();
    view! {
        <unstyled::NumberInput
            value
            min
            max
            label
            disabled
            drag
            face={move |handle: NumberFaceHandle| view! {
                <NumberFace handle shown placeholder={face_placeholder} />
            }}
            field={move |handle: NumberFieldHandle| {
                let NumberFieldHandle {
                    text,
                    editing,
                    on_change,
                    on_submit,
                    on_focus_change,
                    on_key,
                } = handle;
                view! {
                    <TextInput
                        value={text}
                        label={field_label}
                        placeholder
                        disabled={field_disabled}
                        focused={editing}
                        select_on_focus=true
                        on_change={move |typed| on_change.call(typed)}
                        on_submit={move |typed| on_submit.call(typed)}
                        on_focus_change={move |focused| on_focus_change.call(focused)}
                        on_key_override={move |press| on_key.call(press)}
                    />
                }
            }}
            on_change={move |value| on_change.call(value)}
            on_preview={move |value| on_preview.call(value)}
        />
    }
}

#[component]
fn NumberFace(handle: NumberFaceHandle, shown: NodeRef, placeholder: Prop<String>) -> NodeId {
    let NumberFaceHandle {
        text,
        hovered,
        active,
        focused,
        disabled,
    } = handle;
    let theme = use_theme();
    let placeholder = create_memo(move || placeholder.get());
    let string = create_memo(clone!(text placeholder -> move || {
        let text = text.get();
        match text.is_empty() {
            true => placeholder.get(),
            false => text,
        }
    }));
    let color = create_memo(clone!(theme text disabled -> move || {
        match text.get().is_empty() {
            true => theme.text_muted.get(),
            false => ButtonVariant::Secondary.label(&theme, disabled.get()),
        }
    }));
    let fill = create_memo(clone!(theme disabled -> move || {
        ButtonVariant::Secondary.fill(&theme, disabled.get(), hovered.get(), active.get())
    }));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=RADIUS
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame
                height=HEIGHT
                color={fill}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible=true
                padding_horizontal=PADDING_HORIZONTAL
            >
                <Text
                    @node_ref=&shown
                    string={string}
                    font_size=FONT_BODY
                    color={color}
                    align=TextAlign::Center
                    clip=true
                />
            </Frame>
        </Frame>
    }
}

pub fn number_input_field(document: &Document, input: NodeId) -> Option<NodeId> {
    unstyled::number_input_field(document, input)
}

pub fn number_input_text(document: &Document, input: NodeId) -> Option<NodeId> {
    unstyled::number_input_face(document, input)?;
    Some(document.component_state::<FaceText>(input).0.get())
}
