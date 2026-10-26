use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::button::{ButtonSize, ButtonVariant};
use crate::menu_button::PopupButtonFace;
use crate::sheet::AnchoredSheet;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{AnchoredStyle, PopoverHandle, PopupTriggerHandle};
use beui_core::node::NodeId;
use beui_view::reactive::*;

use crate::theme::Density;
use crate::tokens::{Length, Size, Space};

pub const PANEL_PADDING: Space = Space::Lg;
pub const PANEL_MAX_WIDTH: Size = Size::Popover;

#[component]
pub fn Popover(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH)] max_width: Prop<Size>,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(children)] content: RenderFn<PopoverHandle>,
) -> NodeId {
    let max_width = create_memo(move || Some(max_width.get()));
    let surface = popover_surface(max_width);
    view! {
        <unstyled::Popover
            label
            glyph
            icon_only
            disabled
            open
            surface
            trigger={move |handle: PopupTriggerHandle| view! {
                <PopupButtonFace handle variant size />
            }}
        >|handle: PopoverHandle| {
            content.call(handle)
        }</unstyled::Popover>
    }
}

pub fn popover_surface(max_width: impl IntoProp<Option<Size>>) -> AnchoredStyle {
    let max_width = max_width.into_prop();
    AnchoredStyle::new(
        move |content| {
            view! {
                <PopoverPanel max_width={max_width}>{content}</PopoverPanel>
            }
        },
        |handle| {
            view! {
                <AnchoredSheet handle padding=PANEL_PADDING />
            }
        },
    )
}

#[component]
pub fn PopoverPanel(
    #[prop(default = None)] max_width: Prop<Option<Size>>,
    children: Child,
) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Lg
            padding=PANEL_PADDING
            width={max_width.map(|max| max.map_or(Length::INTRINSIC, at_most))}
        >
            {children}
        </Frame>
    }
}

fn at_most(size: Size) -> Length {
    Length::Extent(Extent::at_most(size.resolve(Density::Compact)))
}
