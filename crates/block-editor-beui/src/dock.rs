use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use crate::chrome::sheet_open;

const DOCK_MARGIN: Space = Space::Md;

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
    let content = Render::new(move |()| {
        view! {
            <Frame padding=DOCK_MARGIN>
                <List align=Align::Center spacing=Space::None>
                    {above.map(|above| above.call(())).unwrap_or_default()}
                    <ToolPill @test_id={name}>{tools.call(())}</ToolPill>
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
