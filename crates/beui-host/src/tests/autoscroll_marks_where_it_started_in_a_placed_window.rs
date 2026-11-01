use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use crate::{HostPlacement, Window, WindowHandle, WindowId, WindowPlacement};

const MARKER: Color32 = Color32::from_rgb(200, 10, 30);

fn marker(_: Memo<Direction>) -> NodeId {
    view! {
        <Frame width=20.0 height=20.0 fill={MARKER} />
    }
}

#[test]
fn autoscroll_marks_where_it_started_in_a_placed_window() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(200.0, 200.0)}
                placement={WindowPlacement::Host(HostPlacement("display".to_owned()))}
            >|_: WindowHandle|
                <List spacing=0.0>
                    <Scroll
                        @sizing=ItemSize::Percent(100.0)
                        marker={marker}
                    >
                        <ForEach keys={indices(100)}>|index: usize|
                            <LabelledButton label={format!("Row {index}")} />
                        </ForEach>
                    </Scroll>
                </List>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.context().set_window_hosting(WindowHosting::Placed);
    harness.frame(Vec::new());
    harness.report_window(crate::WindowReport::Rastered {
        window,
        pixels_per_point: 2.0,
    });
    harness.report_window(crate::WindowReport::Moved {
        window,
        origin: pos2(0.0, 120.0),
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let marked = |output: &crate::native::FrameOutput| {
        output.window(window).is_some_and(|shown| {
            shown.shapes().iter().any(
                |shape| matches!(shape, Shape::Rect { color, .. } if *color == MARKER),
            )
        })
    };
    let at = pos2(100.0, 100.0);
    let output = harness.frame_in(
        window,
        vec![
            Input::PointerMoved(at),
            Input::PointerButton {
                pos: at,
                button: PointerButton::Middle,
                pressed: true,
            },
        ],
    );
    assert!(marked(&output), "the marker shows where autoscroll started");
    let shown = output.window(window).expect("the window paints");
    let painted = shown
        .shapes()
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Rect { rect, color, .. } if color == MARKER => Some(rect),
            _ => None,
        })
        .expect("the marker is painted");
    assert!(
        shown.changed && shown.damage().is_some_and(|damage| damage.contains_rect(painted)),
        "the frame that shows the marker damages where it is"
    );
}
