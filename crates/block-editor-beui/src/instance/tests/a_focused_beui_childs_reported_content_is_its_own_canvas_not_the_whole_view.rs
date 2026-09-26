use super::*;
use beui::reactive::{Direction, Frame, ItemSize, List, NodeRef, view};

struct SidebarApp;

impl crate::BeuiApp for SidebarApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let canvas = NodeRef::new();
        editor.content(&canvas);
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame @sizing=ItemSize::Fixed(100.0) />
                <Frame @node_ref={&canvas} @sizing=ItemSize::Percent(100.0) />
            </List>
        }
    }
}

#[test]
fn a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view() {
    let mut session = session::<SidebarApp>(Uuid::new_v4());
    frame(
        &mut session,
        Some(ChildRect {
            x: 300.0,
            y: 250.0,
            width: 120.0,
            height: 90.0,
        }),
        true,
    );

    session.run(EditorRegion::Frame, 1);
    session.run(EditorRegion::Frame, 2);

    let report = session
        .report(EditorRegion::Frame)
        .expect("the frame region reports its content rect");

    assert!(
        report.content.width < 750.0,
        "expected the reported content to be narrowed to the canvas, excluding the 100px sidebar, got {:?}",
        report.content
    );
    assert!(
        report.content.x >= 99.0,
        "expected the reported content to start after the sidebar, got {:?}",
        report.content
    );
}
