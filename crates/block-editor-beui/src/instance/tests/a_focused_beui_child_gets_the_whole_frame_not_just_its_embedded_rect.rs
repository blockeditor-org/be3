use super::*;
use beui::reactive::{Frame, view};

struct RecordingApp;

impl crate::BeuiApp for RecordingApp {
    fn view(_editor: crate::Editor) -> beui::NodeId {
        view! {
            <Frame />
        }
    }
}

#[test]
fn a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect() {
    let mut session = session::<RecordingApp>(Uuid::new_v4());
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

    let report = session
        .report(EditorRegion::Frame)
        .expect("the frame region reports its content rect");

    assert!(
        report.content.width > 400.0,
        "expected the child's own content to span close to the full viewport width, got {:?}",
        report.content
    );
}
