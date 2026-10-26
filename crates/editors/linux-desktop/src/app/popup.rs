use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::popover::{PANEL_MAX_WIDTH, PANEL_PADDING};
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::PopoverHandle;

#[component]
pub(crate) fn BarPopup(
    label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH - 2.0 * PANEL_PADDING)] width: f32,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(children)] content: RenderFn<PopoverHandle>,
) -> NodeId {
    view! {
        <Popover
            label
            glyph
            icon_only
            variant=ButtonVariant::Ghost
            max_width={width + 2.0 * PANEL_PADDING}
            open
        >|handle: PopoverHandle| {
            content.call(handle)
        }</Popover>
    }
}
