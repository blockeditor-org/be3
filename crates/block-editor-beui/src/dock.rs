use beui::NodeId;
use beui::reactive::*;
use beui::styled::theme::BORDER_WIDTH;
use beui::styled::*;
use beui::unstyled::{Edge, Floating};

use crate::chrome::sheet_open;

const DOCK_MARGIN: f32 = 12.0;
const DOCK_PADDING: f32 = 6.0;
const DOCK_SPACING: f32 = 4.0;
const DOCK_RADIUS: f32 = 16.0;

#[component]
pub fn BottomDock(
    anchor: NodeRef,
    #[prop(default = true)] open: Prop<bool>,
    name: String,
    above: Option<Child>,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let sheet = sheet_open();
    let open = create_memo(move || open.get() && !sheet.get());
    let theme = use_theme();
    view! {
        <Floating anchor={anchor} edge=Edge::Bottom open={open}>
            <Frame padding=DOCK_MARGIN>
                <List align=Align::Center spacing=0.0>
                    <Frame children={above} />
                    <Frame
                        fill={theme.surface_raised}
                        outline={create_memo(move || Some(Outline::new(theme.border.get(), BORDER_WIDTH)))}
                        radius=DOCK_RADIUS
                        padding=DOCK_PADDING
                        @test_id={name}
                    >
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=DOCK_SPACING
                            children={children}
                        />
                    </Frame>
                </List>
            </Frame>
        </Floating>
    }
}
