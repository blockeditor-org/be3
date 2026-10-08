use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{Prop, Render, component, view};
use block_editor_beui::beui::styled::popover::{PANEL_MAX_WIDTH, PANEL_PADDING};
use block_editor_beui::beui::styled::{ButtonVariant, Popover};
use block_editor_beui::beui::unstyled::PopoverHandle;

#[component]
pub(crate) fn BarPopup(
    label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH - 2.0 * PANEL_PADDING)] width: f32,
    #[prop(children)] content: Render<PopoverHandle>,
) -> NodeId {
    view! {
        <Popover
            label
            glyph
            icon_only
            variant=ButtonVariant::Ghost
            max_width={width + 2.0 * PANEL_PADDING}
        >
            {move |handle: PopoverHandle| content.call(handle)}
        </Popover>
    }
}
