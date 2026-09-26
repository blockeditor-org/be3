use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use beui::ItemSize;
use beui::reactive::{Frame, List, NodeRef, view};
use beui::unstyled::Floating;
use block_plugin_api::ChildMode;

const BLOCK: Uuid = Uuid::from_u128(1);
const BLOCK_TYPE: Uuid = Uuid::from_u128(2);
const PANEL: f32 = 120.0;

struct OverlaidApp;

impl crate::BeuiApp for OverlaidApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let child = NodeRef::new();
        view! {
            <Frame>
                <List spacing=0.0>
                    <ChildBlock
                        @node_ref={&child}
                        @sizing=ItemSize::Percent(100.0)
                        editor={editor}
                        block={Some(ChildTarget::new(BLOCK, BLOCK_TYPE))}
                        mode=ChildMode::Live
                        own_frame=true
                        on_state={move |_: ChildState| {}}
                    />
                    <Floating anchor={child}>
                        <Frame width=PANEL height=PANEL />
                    </Floating>
                </List>
            </Frame>
        }
    }
}

#[test]
fn an_open_beui_overlay_is_reported_over_the_child_it_covers() {
    let mut session = session::<OverlaidApp>(BLOCK_TYPE);
    frame(&mut session, None, false);

    session.run(EditorRegion::Frame, 1);
    session.run(EditorRegion::Frame, 2);

    let report = session
        .report(EditorRegion::Frame)
        .expect("the frame region reports");
    let floating = report
        .floating
        .first()
        .expect("an open overlay must be reported so the host blits it over the children");
    assert_eq!(
        (floating.width, floating.height),
        (PANEL, PANEL),
        "the reported rectangle is the one the overlay was laid out at"
    );
    let occluder = session
        .occluders(EditorRegion::Frame)
        .first()
        .expect("an open overlay must be withheld from the child under it");
    assert_eq!(
        (occluder.rect.width, occluder.rect.height),
        (PANEL, PANEL),
        "the occluder covers the same rectangle the overlay was laid out at"
    );
    assert!(
        occluder.after as usize >= session.placed_children(EditorRegion::Frame).len(),
        "the overlay occludes every child placed under it"
    );
}
