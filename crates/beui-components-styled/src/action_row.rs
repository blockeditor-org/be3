use beui_core::icons::Glyph;
use beui_core::image::Image;
use beui_macros::{component, view};

use crate::row::{RowFace, RowSize, RowTrailing};
use crate::text::TextColor;
use beui_components_unstyled as unstyled;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn ActionRow(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] glyph_color: Prop<Option<TextColor>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = RowTrailing::None)] trailing: Prop<RowTrailing>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: bool,
    action: Option<Action>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            glyph
            action
            disabled
            on_click={on_click}
        >|handle: unstyled::ButtonHandle|
            <RowFace
                hovered={handle.hovered}
                active={handle.active}
                focused={handle.focused}
                disabled={handle.disabled}
                danger
                size=RowSize::Touch
                label={handle.label}
                glyph={handle.glyph}
                glyph_color
                image
                detail
                trailing
            />
        </unstyled::Button>
    }
}
