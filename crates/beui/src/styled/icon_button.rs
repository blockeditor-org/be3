use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{ClickCallback, Frame, Prop, clone, create_memo};
use crate::styled::button::ButtonVariant;
use crate::styled::text::IconSized;
use crate::styled::theme::{BORDER_WIDTH, ICON_SIZE, RADIUS, use_theme};
use crate::unstyled;

const PADDING: f32 = 8.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn IconButton(
    glyph: Prop<String>,
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = false)] disabled: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::Button);
        node.set_label(label.get());
        node
    });
    let disabled = create_memo(move || disabled.get());
    let face = disabled.clone();
    let glyph = create_memo(move || glyph.get());
    view! {
        <unstyled::Button
            disabled
            accessibility
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <IconButtonFace handle variant glyph={glyph.clone()} disabled={face.clone()} />
            }}
        />
    }
}

#[component]
fn IconButtonFace(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    glyph: Prop<String>,
    disabled: Prop<bool>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let off = create_memo(move || disabled.get());
    let fill_color = create_memo(clone!(theme off -> move || {
        variant.fill(&theme, off.get(), hovered.get(), active.get())
    }));
    let icon_color = create_memo(clone!(theme off -> move || variant.label(&theme, off.get())));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 4}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame
                color={fill_color}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                <IconSized glyph={glyph} font_size=ICON_SIZE color={icon_color} />
            </Frame>
        </Frame>
    }
}
