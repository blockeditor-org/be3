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
    let mut session = EditorSession::beui::<RecordingApp>(
        Rc::new(Vec::new()),
        EditorInstanceId(0),
        Waker::default(),
    );
    session.regions.insert(
        EditorRegion::Frame,
        RegionState {
            placement: Some(ScreenPlacement {
                screen: block_plugin_api::ScreenId(0),
                instance: EditorInstanceId(0),
                region: EditorRegion::Frame,
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                scale_factor_millis: 1000,
            }),
            metrics: Some(ViewportMetrics {
                logical_width: 800.0,
                logical_height: 600.0,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
            }),
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content: Some(ChildRect {
                    x: 300.0,
                    y: 250.0,
                    width: 120.0,
                    height: 90.0,
                }),
                trail: vec!["Canvas".to_owned(), "Pan and Zoom".to_owned()],
            }),
            ..Default::default()
        },
    );

    session.run_beui(EditorRegion::Frame, 1);

    let report = session
        .regions
        .get(&EditorRegion::Frame)
        .and_then(|state| state.report.as_ref())
        .expect("the frame region reports its content rect");

    assert!(
        report.content.width > 400.0,
        "expected the child's own content to span close to the full viewport width, got {:?}",
        report.content
    );
}
