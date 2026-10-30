use beui::Direction;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::popover::PANEL_MAX_WIDTH;
use block_editor_beui::beui::styled::*;

use super::bar::bar_direction;

#[component]
pub(crate) fn BarPopup(
    label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = PANEL_MAX_WIDTH)] width: Prop<Size>,
    #[prop(default = false)] open: Binding<bool>,
    leading: Option<Child>,
    #[prop(children)] content: RenderFn<PopoverHandle>,
) -> NodeId {
    let direction = bar_direction();
    let placement = create_memo(move || match direction.get() {
        Direction::Horizontal => PopoverPlacement::Below,
        Direction::Vertical => PopoverPlacement::Beside,
    });
    view! {
        <Popover
            label
            glyph
            icon_only
            variant=ButtonVariant::Ghost
            max_width=width
            open
            placement
            leading
        >|handle: PopoverHandle| {
            content.call(handle)
        }</Popover>
    }
}
