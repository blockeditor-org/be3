use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::focus_ring::FocusRing;
use crate::text::{Text, TextColor};
use crate::text_input::TextInput;
use crate::theme::{BORDER_WIDTH, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{NumberDrag, NumberFaceHandle, NumberFieldHandle};
use beui_core::base::TextAlign;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 10.0;
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
    set_component_state(FaceText(shown));
    let (field_label, field_disabled) = (label.clone(), disabled);
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
                } = handle;
                view! {
                    <TextInput
                        value={text}
                        label={field_label}
                        placeholder
                        disabled={field_disabled}
                        focused={editing}
                        select_on_focus=true
                        on_change={on_change}
                        on_submit={on_submit}
                        on_focus_change={on_focus_change}
                    />
                }
            }}
            on_change={on_change}
            on_preview={on_preview}
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
    let string = create_memo(move || {
        let text = text.get();
        match text.is_empty() {
            true => placeholder.get(),
            false => text,
        }
    });
    let color = create_memo(move || match text.get().is_empty() {
        true => TextColor::Muted,
        false => ButtonVariant::Secondary.label(disabled.get()),
    });
    let fill = create_memo(move || {
        ButtonVariant::Secondary.fill(&theme, disabled.get(), hovered.get(), active.get())
    });
    view! {
        <FocusRing focused offset=FOCUS_RING_OFFSET>
            <Frame
                height=HEIGHT
                fill=fill
                outline={create_memo(move || Some(Outline::new(theme.border.get(), BORDER_WIDTH)))}
                radius=RADIUS
                padding={Sides::horizontal(PADDING_HORIZONTAL)}
            >
                <Text
                    @node_ref=&shown
                    string={string}
                    color={color}
                    align=TextAlign::Center
                    clip=true
                />
            </Frame>
        </FocusRing>
    }
}

pub fn number_input_field(document: &Document, input: NodeId) -> Option<NodeId> {
    unstyled::number_input_field(document, input)
}

pub fn number_input_text(document: &Document, input: NodeId) -> Option<NodeId> {
    unstyled::number_input_face(document, input)?;
    Some(document.component_state::<FaceText>(input).0.get())
}
