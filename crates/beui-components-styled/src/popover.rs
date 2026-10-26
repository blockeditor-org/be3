use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::button::{ButtonSize, ButtonVariant};
use crate::menu_button::PopupButtonFace;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{PopoverHandle, PopupTriggerHandle};
use beui_core::node::NodeId;
use beui_view::reactive::*;

pub const PANEL_PADDING: f32 = 12.0;
pub const PANEL_MAX_WIDTH: f32 = 320.0;

#[component]
pub fn Popover(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH)] max_width: Prop<f32>,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(children)] content: Render<PopoverHandle>,
) -> NodeId {
    let max_width = create_memo(move || Some(max_width.get()));
    view! {
        <unstyled::Popover
            label
            glyph
            icon_only
            disabled
            open
            trigger={move |handle: PopupTriggerHandle| view! {
                <PopupButtonFace handle variant size />
            }}
        >|handle: PopoverHandle|
            <PopoverPanel max_width={max_width}>{content.call(handle)}</PopoverPanel>
        </unstyled::Popover>
    }
}

#[component]
pub fn PopoverPanel(
    #[prop(default = None)] max_width: Prop<Option<f32>>,
    children: Child,
) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Lg
            padding=PANEL_PADDING
            width={max_width.map(|max| Extent { max, ..Extent::INTRINSIC })}
        >
            {children}
        </Frame>
    }
}
