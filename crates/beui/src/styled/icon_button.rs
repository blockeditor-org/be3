use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{ClickCallback, Frame, Prop, clone, create_memo};
use crate::styled::button::ButtonVariant;
use crate::styled::text::IconSized;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;

const PADDING: f32 = 8.0;
const COMPACT_PADDING: f32 = 2.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;
const COMPACT_FOCUS_RING_OFFSET: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconButtonSize {
    Compact,
    Regular,
}

impl IconButtonSize {
    fn padding(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_PADDING,
            IconButtonSize::Regular => PADDING,
        }
    }

    fn glyph(self) -> f32 {
        match self {
            IconButtonSize::Compact => FONT_BODY,
            IconButtonSize::Regular => ICON_SIZE,
        }
    }

    fn focus_ring_offset(self) -> f32 {
        match self {
            IconButtonSize::Compact => COMPACT_FOCUS_RING_OFFSET,
            IconButtonSize::Regular => FOCUS_RING_OFFSET,
        }
    }
}

#[component]
pub fn IconButton(
    glyph: Prop<String>,
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = IconButtonSize::Regular)] size: IconButtonSize,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let accessibility = create_memo(clone!(label -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(label.get());
        node
    }));
    let disabled = create_memo(move || disabled.get());
    let face = disabled.clone();
    let glyph = create_memo(move || glyph.get());
    let label = create_memo(clone!(label -> move || label.get()));
    view! {
        <unstyled::Button
            disabled
            capture_presses
            accessibility
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <Tooltip label={label.clone()}>
                    <IconButtonFace
                        handle
                        variant
                        size
                        glyph={glyph.clone()}
                        disabled={face.clone()}
                    />
                </Tooltip>
            }}
        />
    }
}

#[component]
fn IconButtonFace(
    handle: unstyled::ButtonHandle,
    variant: ButtonVariant,
    size: IconButtonSize,
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
            outline_offset={size.focus_ring_offset()}
            outline_visible={focused}
        >
            <Frame
                color={fill_color}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                radius=RADIUS
                outline_visible={variant == ButtonVariant::Secondary}
                padding_horizontal={size.padding()}
                padding_vertical={size.padding()}
            >
                <IconSized glyph={glyph} font_size={size.glyph()} color={icon_color} />
            </Frame>
        </Frame>
    }
}
