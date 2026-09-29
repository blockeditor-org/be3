use beui::NodeId;
use beui::reactive::{
    Align, Children, Direction, Frame, List, ListChild, NodeRef, Prop, clone, component,
    create_memo, view,
};
use beui::styled::theme::BORDER_WIDTH;
use beui::styled::use_theme;
use beui::unstyled::{Edge, Floating};

use crate::chrome::sheet_open;

const DOCK_MARGIN: f32 = 12.0;
const DOCK_PADDING: f32 = 6.0;
const DOCK_SPACING: f32 = 4.0;
const DOCK_RADIUS: u8 = 16;

#[component]
pub fn BottomDock(
    anchor: NodeRef,
    #[prop(default = true)] open: Prop<bool>,
    name: String,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let sheet = sheet_open();
    let open = create_memo(clone!(sheet -> move || open.get() && !sheet.get()));
    let theme = use_theme();
    view! {
        <Floating anchor={anchor} edge=Edge::Bottom open={open}>
            <Frame padding_vertical=DOCK_MARGIN padding_horizontal=DOCK_MARGIN>
                <Frame
                    color={theme.surface_raised.clone()}
                    outline={theme.border.clone()}
                    outline_width=BORDER_WIDTH
                    outline_visible=true
                    radius=DOCK_RADIUS
                    padding_horizontal=DOCK_PADDING
                    padding_vertical=DOCK_PADDING
                    @test_id={name}
                >
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=DOCK_SPACING
                        children={children}
                    />
                </Frame>
            </Frame>
        </Floating>
    }
}
