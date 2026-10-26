use beui_components_styled::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::unstyled::{DockSwitchHandle, DockSwitchRowHandle, TabId};
use beui_components_styled::list_row::ListRow;
use beui_components_styled::text::{Icon, Text, TextColor};
use beui_core::base::{Align, Direction};
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use beui_components_styled::List;
use beui_components_styled::tokens::{Padding, Size, Space};
use beui_view::reactive::*;

const PANEL_WIDTH: Size = Size::Dialog;
const PANEL_PADDING: Space = Space::Sm;
const ROW_SPACING: Space = Space::Xs2;
const ROW_PADDING: Space = Space::Sm;
const ICON_SPACING: Space = Space::Md;

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
        tab,
        title,
        icon,
        chosen,
        pick,
    } = handle;
    view! {
        <ListRow
            @test_id={format!("dock.switch.{}", tab.value())}
            selected={chosen}
            on_click={pick}
        >
            <Frame padding={Padding::vertical(ROW_PADDING)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=ICON_SPACING>
                    <Text color=TextColor::Accent align=TextAlign::Center>
                        <Icon glyph={icon} />
                    </Text>
                    <Text overflow=Overflow::Ellipsis>{title}</Text>
                </List>
            </Frame>
        </ListRow>
    }
}
