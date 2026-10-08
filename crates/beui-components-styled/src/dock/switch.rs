use beui_macros::{component, view};

use crate::list_row::ListRow;
use crate::text::Icon;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, FONT_BODY, use_theme};
use beui_components_unstyled::{DockSwitchHandle, DockSwitchRowHandle, TabId};
use beui_core::base::{Align, Direction};
use beui_core::node::NodeId;
use beui_view::reactive::{ForEach, Frame, List, Text};

const PANEL_WIDTH: f32 = 360.0;
const PANEL_PADDING: f32 = 6.0;
const ROW_SPACING: f32 = 2.0;
const ROW_PADDING: f32 = 6.0;
const ICON_SPACING: f32 = 10.0;

#[component]
pub(crate) fn DockSwitchPanel(handle: DockSwitchHandle) -> NodeId {
    let DockSwitchHandle { tabs, row, .. } = handle;
    let theme = use_theme();
    view! {
        <Frame
            width=PANEL_WIDTH
            radius=CARD_RADIUS
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            padding_horizontal=PANEL_PADDING
            padding_vertical=PANEL_PADDING
        >
            <List spacing=ROW_SPACING>
                <ForEach keys={tabs}>
                    {move |tab: TabId| view! {
                        <DockSwitchRow handle={row.call(tab)} />
                    }}
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
    let theme = use_theme();
    view! {
        <ListRow
            @test_id={format!("dock.switch.{}", tab.value())}
            selected={chosen}
            on_click={move || pick.call()}
        >
            <Frame padding_vertical=ROW_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ICON_SPACING>
                    <Icon glyph={icon} color={theme.accent.clone()} />
                    <Text
                        string={title}
                        font_size=FONT_BODY
                        color={theme.text.clone()}
                        ellipsis=true
                    />
                </List>
            </Frame>
        </ListRow>
    }
}
