use beui_core::icons::Glyph;
use beui_core::image::Image;
use beui_macros::{component, view};

use crate::row::{RowFace, RowSize, RowTrailing};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::ButtonHandle;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn ListRow(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = RowTrailing::None)] trailing: Prop<RowTrailing>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_click: ClickCallback,
    on_activate: ClickCallback,
    children: Option<Child>,
) -> NodeId {
    let activates = !on_activate.is_empty();
    let selected = create_memo(move || selected.get());
    view! {
        <unstyled::ListRow
            selected={selected}
            disabled
            activates
            on_click={on_click}
            on_activate={on_activate}
        >|handle: ButtonHandle|
            <RowFace
                hovered={handle.hovered}
                active={handle.active}
                focused={handle.focused}
                disabled={handle.disabled}
                selected={selected}
                size=RowSize::Compact
                label
                detail
                glyph
                image
                trailing
                children
            />
        </unstyled::ListRow>
    }
}
