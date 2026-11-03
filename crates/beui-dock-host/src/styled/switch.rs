use beui_components_styled::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::unstyled::{DockSwitchHandle, DockSwitchRowHandle};
use beui_components_styled::List;
use beui_components_styled::list_row::ListRow;
use beui_components_styled::row::RowSize;
use beui_components_styled::text::TextColor;
use beui_components_styled::tokens::{Size, Space};
use beui_dock::TabId;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

const PANEL_WIDTH: Size = Size::Dialog;
const PANEL_PADDING: Space = Space::Sm;
const ROW_SPACING: Space = Space::Xs2;

#[component]
pub(crate) fn DockSwitchPanel(handle: DockSwitchHandle) -> NodeId {
    let DockSwitchHandle { tabs, row, .. } = handle;
    view! {
        <Frame
            width=PANEL_WIDTH
            radius=Radius::Lg
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            padding=PANEL_PADDING
        >
            <List spacing=ROW_SPACING>
                <ForEach keys={tabs}>|tab: TabId|
                    <DockSwitchRow handle={row.call(tab)} />
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn DockSwitchRow(handle: DockSwitchRowHandle) -> NodeId {
    let DockSwitchRowHandle {
        test_key,
        title,
        icon,
        chosen,
        pick,
        ..
    } = handle;
    view! {
        <ListRow
            @test_id={format!("dock.switch.{test_key}")}
            label={title}
            glyph={icon}
            glyph_color={Some(TextColor::Accent)}
            size=RowSize::Large
            selected={chosen}
            on_click={pick}
        />
    }
}
