use beui::NodeId;
use beui::reactive::*;
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
    view! {
        <Floating anchor={anchor} edge=Edge::Bottom open={open}>
            <Frame padding=DOCK_MARGIN>
                <List align=Align::Center spacing=0.0>
                    <Frame children={above} />
                    <Frame
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius={Radius::custom(DOCK_RADIUS)}
                        @test_id={name}
                    >
                        <List direction=Direction::Horizontal spacing=0.0>
                            <Scroll
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                direction=Direction::Horizontal
                                fit=true
                                tab_stop=false
                            >
                                <Frame padding=DOCK_PADDING>
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=DOCK_SPACING
                                        children={children}
                                    />
                                </Frame>
                            </Scroll>
                        </List>
                    </Frame>
                </List>
            </Frame>
        </Floating>
    }
}
