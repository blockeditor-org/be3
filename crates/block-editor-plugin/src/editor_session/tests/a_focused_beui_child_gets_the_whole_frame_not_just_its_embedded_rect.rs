use super::*;
use std::cell::Cell;

thread_local! {
    static LAST_RECT: Cell<Option<beui::Rect>> = const { Cell::new(None) };
}

#[derive(Default)]
struct RecordingApp;

impl crate::BeuiApp for RecordingApp {
    fn frame(&mut self, _context: &beui::Context, rect: beui::Rect) {
        LAST_RECT.with(|cell| cell.set(Some(rect)));
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

    let rect = LAST_RECT
        .with(|cell| cell.get())
        .expect("the beui app's frame() was never called");

    assert_eq!(rect.min.x, 0.0);
    assert_eq!(rect.max.x, 800.0);
    assert!(
        rect.height() > 400.0,
        "expected the child to be given close to the full viewport height, got {rect:?}"
    );
}
