use crate::frame::{Border, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::Overflow;

use crate::button::ButtonVariant;
use crate::focus_ring::FocusRing;
use crate::text::{Text, TextColor};
use crate::text_input::TextInput;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{NumberDrag, NumberFaceHandle, NumberFieldHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::TextAlign;

use crate::tokens::{Padding, Size, Space, Stroke};

const HEIGHT: Size = Size::Field;
const PADDING_HORIZONTAL: Space = Space::Md;

#[component]
pub fn NumberInput(
    #[prop(default = ComponentRef::new())] api: ComponentRef<NumberInputTextApi>,
    value: Binding<f64>,
    #[prop(default = f64::NEG_INFINITY)] min: f64,
    #[prop(default = f64::INFINITY)] max: f64,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = NumberDrag::Linear { speed: 1.0 })] drag: NumberDrag,
    #[prop(default = None)] decimals: Prop<Option<u32>>,
    on_preview: Callback<Option<f64>>,
    on_scrub: Callback<bool>,
) -> NodeId {
    let shown = NodeRef::new();
    let inner = ComponentRef::new();
    api.fill(NumberInputTextApi { inner, shown });
    let (field_label, field_disabled) = (label.clone(), disabled);
    let face_placeholder = placeholder.clone();
    view! {
        <unstyled::NumberInput
            api=inner
            value
            min
            max
            label
            disabled
            drag
            decimals
            face={move |handle: NumberFaceHandle| view! {
                <NumberFace handle shown placeholder={face_placeholder} />
            }}
            field={move |handle: NumberFieldHandle| {
                let NumberFieldHandle {
                    text,
                    editing,
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
                        on_submit={on_submit}
                        on_focus_change={on_focus_change}
                    />
                }
            }}
            on_preview={on_preview}
            on_scrub={on_scrub}
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
        ButtonVariant::Secondary.fill(disabled.get(), hovered.get(), active.get())
    });
    view! {
        <FocusRing focused offset=Stroke::Focus>
            <Frame
                height=HEIGHT
                fill={fill}
                border={Border::DEFAULT}
                radius=Radius::Md
                padding={Padding::horizontal(PADDING_HORIZONTAL)}
            >
                <Text
                    @node_ref=&shown
                    color={color}
                    align=TextAlign::Center
                    overflow=Overflow::Ellipsis
                >
                    {string}
                </Text>
            </Frame>
        </FocusRing>
    }
}

#[derive(Clone, Copy)]
pub struct NumberInputTextApi {
    inner: ComponentRef<unstyled::NumberInputApi>,
    shown: NodeRef,
}

impl NumberInputTextApi {
    pub fn text(&self) -> Option<NodeId> {
        self.inner.get().face()?;
        Some(self.shown.get())
    }
}
