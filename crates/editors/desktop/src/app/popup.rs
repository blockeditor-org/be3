use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::popover::PANEL_MAX_WIDTH;
use block_editor_beui::beui::styled::*;

#[component]
pub(crate) fn BarPopup(
    label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH)] width: Size,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(children)] content: RenderFn<PopoverHandle>,
) -> NodeId {
    view! {
        <Popover
            label
            glyph
            icon_only
            variant=ButtonVariant::Ghost
            max_width=width
            open
        >|handle: PopoverHandle| {
            content.call(handle)
        }</Popover>
    }
}
