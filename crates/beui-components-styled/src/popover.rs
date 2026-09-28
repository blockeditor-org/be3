use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{PopoverHandle, PopoverTriggerHandle};
use beui_core::node::NodeId;
use beui_view::reactive::{Callback, Child, Frame, Prop, Render, clone, create_memo};

pub const PANEL_PADDING: f32 = 12.0;

#[component]
pub fn Popover(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(children)] content: Render<PopoverHandle>,
    on_open_change: Callback<bool>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let named = create_memo(move || !icon_only.get());
    let face_label = create_memo(clone!(label named -> move || match named.get() {
        true => label.get(),
        false => String::new(),
    }));
    let face_disabled = create_memo(clone!(disabled -> move || disabled.get()));
    view! {
        <unstyled::Popover
            label={label.clone()}
            disabled
            on_open_change={move |open| on_open_change.call(open)}
            trigger={move |handle: PopoverTriggerHandle| {
                let PopoverTriggerHandle {
                    open,
                    hovered,
                    active,
                    focused,
                    ..
                } = handle;
                let quiet = create_memo(clone!(named -> move || named.get() || open.get()));
                view! {
                    <Tooltip label disabled={quiet}>
                        <ButtonFace
                            handle={unstyled::ButtonHandle { hovered, active, focused }}
                            variant
                            label={face_label}
                            glyph
                            disabled={face_disabled}
                        />
                    </Tooltip>
                }
            }}
        >
            {move |handle: PopoverHandle| view! {
                <PopoverPanel>{content.call(handle)}</PopoverPanel>
            }}
        </unstyled::Popover>
    }
}

#[component]
pub fn PopoverPanel(children: Child) -> NodeId {
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
        >
            {children}
        </Frame>
    }
}
