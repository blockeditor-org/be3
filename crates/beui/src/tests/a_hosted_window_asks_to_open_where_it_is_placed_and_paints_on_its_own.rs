use super::*;
use crate::{HostPlacement, WindowId, WindowPlacement, WindowRequest};
use crate::{Window, WindowHandle};

#[test]
fn a_hosted_window_asks_to_open_where_it_is_placed_and_paints_on_its_own() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let placed = WindowPlacement::Host(HostPlacement("on the left wall".to_owned()));
    let clicks = create_signal(0);
    let content = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(120.0, 80.0)}
                at={pos2(50.0, 40.0)}
                placement={WindowPlacement::Host(HostPlacement("on the left wall".to_owned()))}
            >|_: WindowHandle|
                <Interactive on_click={move || clicks.update(|clicks| *clicks += 1)}>
                    <Frame @node_ref=&content fill={Color32::WHITE} />
                </Interactive>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        harness.take_window_requests(),
        [WindowRequest::Open {
            window,
            placement: placed,
            size: vec2(120.0, 80.0),
            role: crate::WindowRole::Normal,
        }],
        "the window asks its host to open it once, where it was placed"
    );
    assert_eq!(
        harness.document().node_rect(content.get()),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(120.0, 80.0))),
        "a window lays its content out in its own space, from its own corner"
    );
    let output = harness.frame(Vec::new());
    let shown = output
        .window(window)
        .expect("the window paints an output of its own");
    assert_eq!(shown.size, vec2(120.0, 80.0));
    let white = |shapes: Vec<Shape>| {
        shapes
            .iter()
            .any(|shape| matches!(shape, Shape::Rect { color, .. } if *color == Color32::WHITE))
    };
    assert!(white(shown.shapes()), "the content paints into its window");
    assert!(
        !white(output.main().shapes()),
        "and none of it into the main window"
    );

    harness.frame_in(
        window,
        vec![
            Input::PointerMoved(pos2(10.0, 10.0)),
            Input::PointerButton {
                pos: pos2(10.0, 10.0),
                button: PointerButton::Primary,
                pressed: true,
            },
            Input::PointerButton {
                pos: pos2(10.0, 10.0),
                button: PointerButton::Primary,
                pressed: false,
            },
        ],
    );
    assert_eq!(
        clicks.get_untracked(),
        1,
        "a press in the window reaches its content"
    );
    harness.click(pos2(60.0, 50.0));
    assert_eq!(
        clicks.get_untracked(),
        1,
        "a press in the main window where the overlay would have been reaches nothing in it"
    );
}
