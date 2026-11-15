use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::use_dock_bar;
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::popover::{PANEL_MAX_WIDTH, PANEL_PADDING};
use be_sdk_beui::beui::styled::*;
use beui::Direction;

use super::bar::{bar_bounds, bar_direction};

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
    let bar = use_dock_bar();
    let band = bar.map(|bar| bar.surface());
    let bounds = bar_bounds();
    let max_length = create_memo(move || {
        let inset = 2.0 * (PANEL_PADDING.px() + Border::DEFAULT.width.px());
        bounds
            .get()
            .map(|bounds| Size::Custom((bounds.height() - inset).max(0.0)))
    });
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
            band
            bounds
            leading
        >|handle: PopoverHandle|
            <Scroll max_length bleed=Bleed::POPOVER>{content.call(handle)}</Scroll>
        </Popover>
    }
}
