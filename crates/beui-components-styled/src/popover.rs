use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{PopoverHandle, PopoverTriggerHandle};
use beui_core::node::NodeId;
use beui_view::reactive::{Callback, Child, Frame, Prop, Render, clone, create_memo};

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
            on_open_change={move |open| on_open_change.call(open)}
            trigger={move |handle: PopoverTriggerHandle| {
                let PopoverTriggerHandle { open, button } = handle;
                let quiet = create_memo(clone!(icon_only -> move || !icon_only.get() || open.get()));
                view! {
                    <Tooltip label={button.label.clone()} disabled={quiet}>
                        <ButtonFace handle={button} variant icon_only={icon_only.clone()} />
                    </Tooltip>
                }
            }}
        >
            {move |handle: PopoverHandle| view! {
                <PopoverPanel max_width={max_width.clone()}>{content.call(handle)}</PopoverPanel>
            }}
        </unstyled::Popover>
    }
}

#[component]
pub fn PopoverPanel(
    #[prop(default = None)] max_width: Prop<Option<f32>>,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=CARD_RADIUS
            padding_horizontal=PANEL_PADDING
            padding_vertical=PANEL_PADDING
            max_width
        >
            {children}
        </Frame>
    }
}
