use super::*;
use crate::primitives::{Window, WindowHandle};
use crate::{HostPlacement, WindowId, WindowReport};

#[test]
fn a_hosts_report_of_a_windows_size_and_place_reaches_the_app() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let heard = create_signal(Vec::<WindowReport>::new());
    let content = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(120.0, 80.0)}
                on_report={move |report: WindowReport| heard.update(|heard| heard.push(report))}
            >|_: WindowHandle|
                <Frame @node_ref=&content />
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    heard.set(Vec::new());

    let pose = HostPlacement("two metres ahead".to_owned());
    harness.report_window(WindowReport::Resized {
        window,
        size: vec2(200.0, 150.0),
    });
    harness.report_window(WindowReport::Placed {
        window,
        placement: pose.clone(),
    });
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());

    assert_eq!(
        heard.get_untracked(),
        [
            WindowReport::Resized {
                window,
                size: vec2(200.0, 150.0),
            },
            WindowReport::Placed {
                window,
                placement: pose.clone(),
            },
        ],
        "the window hears what its host reports, to store the place it hands back"
    );
    assert_eq!(
        harness.document().node_rect(content.get()),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        "the content is laid out at the size the host gave the window"
    );
    assert_eq!(
        output.window(window).map(|shown| shown.size),
        Some(vec2(200.0, 150.0))
    );
    assert_eq!(harness.context().window_placement(window), Some(pose));
}
