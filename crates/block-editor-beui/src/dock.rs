use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use crate::chrome::sheet_open;

const DOCK_MARGIN: Space = Space::Lg;
const DOCK_PADDING: Space = Space::Sm;
const DOCK_SPACING: Space = Space::Xs;

#[component]
pub fn BottomDock(
    anchor: NodeRef,
    #[prop(default = true)] open: Prop<bool>,
    name: String,
    above: Option<Render<(), Children<ListChild>>>,
    #[prop(children)] tools: Render<(), Children<ListChild>>,
) -> NodeId {
    let sheet = sheet_open();
    let open = create_memo(move || open.get() && !sheet.get());
    let row = NodeRef::new();
    arrow_focus(row, Direction::Horizontal);
    let content = Render::new(move |()| {
        view! {
            <Frame padding=DOCK_MARGIN>
                <List align=Align::Center spacing=Space::None>
                    {above.map(|above| above.call(())).unwrap_or_default()}
                    <Frame
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius=Radius::Lg
                        @test_id={name}
                    >
                        <List direction=Direction::Horizontal spacing=Space::None>
                            <Scroll
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                direction=Direction::Horizontal
                                fit=true
                            >
                                <Frame padding=DOCK_PADDING>
                                    <List
                                        @node_ref=&row
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=DOCK_SPACING
                                    >
                                        {tools.call(())}
                                    </List>
                                </Frame>
                            </Scroll>
                        </List>
                    </Frame>
                </List>
            </Frame>
        }
    })
    .once();
    view! {
        <Floating anchor={anchor} edge=Edge::Bottom open={open} keep_alive=true>
            {content.call(())}
        </Floating>
    }
}
