use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::button::{ButtonSize, ButtonVariant};
use crate::menu_button::PopupButtonFace;
use crate::sheet::AnchoredSheet;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{AnchoredStyle, Dropdown, PopoverHandle, PopupTriggerHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

use crate::theme::POINTER_REACH;
use crate::tokens::{Length, Size, Space};
use beui_view::types::Extent;
use beui_view::types::{Placement, Pos2, Rect};

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
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = true)] keep_alive: bool,
    /// Points an arrow at what it opened from while it is above or below it; give the
    /// placement a gap for the arrow to fill.
    #[prop(default = false)] arrow: Prop<bool>,
    band: Option<NodeRef>,
    leading: Option<Child>,
    #[prop(children)] content: RenderFn<PopoverHandle, NodeId>,
) -> NodeId {
    let max_width = create_memo(move || Some(max_width.get()));
    let surface = popover_surface(max_width);
    let beak = create_memo(move || match arrow.get() {
        true => POINTER_REACH,
        false => 0.0,
    });
    view! {
        <unstyled::Popover
            label
            glyph
            icon_only
            disabled
            open
            at
            placement
            bounds
            keep_alive
            beak={beak}
            band
            surface
            trigger={move |handle: PopupTriggerHandle| view! {
                <PopupButtonFace handle variant size leading />
            }}
        >|handle: PopoverHandle| {
            content.call(handle)
        }</unstyled::Popover>
    }
}

pub fn popover_surface<M1>(max_width: impl IntoPropValue<Prop<Option<Size>>, M1>) -> AnchoredStyle {
    let max_width = max_width.into_prop_value();
    AnchoredStyle::new(
        move |dropdown: Dropdown| {
            view! {
                <PopoverPanel pointer={dropdown.pointer} max_width={max_width}>
                    {dropdown.content}
                </PopoverPanel>
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
    #[prop(default = NodeRef::new())] pointer: NodeRef,
    children: Child,
) -> NodeId {
    view! {
        <Frame
            @node_ref=&pointer
            fill=ColorRole::Surface
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
    Length::Extent(Extent::at_most(size.px()))
}
