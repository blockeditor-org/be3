use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{PopoverHandle, PopoverTriggerHandle};
use beui_core::node::NodeId;
use beui_view::reactive::*;

pub const PANEL_PADDING: f32 = 12.0;
pub const PANEL_MAX_WIDTH: f32 = 320.0;

#[component]
pub fn Popover(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH)] max_width: Prop<f32>,
    #[prop(children)] content: Render<PopoverHandle>,
    on_open_change: Callback<bool>,
) -> NodeId {
    let icon_only = create_memo(move || icon_only.get());
    let max_width = create_memo(move || Some(max_width.get()));
    view! {
        <unstyled::Popover
            label
            glyph
            disabled
            on_open_change={on_open_change}
            trigger={move |handle: PopoverTriggerHandle| {
                let PopoverTriggerHandle { open, button } = handle;
                let quiet = create_memo(move || !icon_only.get() || open.get());
                view! {
                    <Tooltip label={button.label} disabled={quiet}>
                        <ButtonFace handle={button} variant icon_only={icon_only} />
                    </Tooltip>
                }
            }}
        >
            |handle: PopoverHandle| <PopoverPanel max_width={max_width}>{content.call(handle)}</PopoverPanel>
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
