use super::*;
use crate::{Window, WindowHandle};
use crate::{WindowEdge, WindowId, WindowReport, WindowRequest};

#[test]
fn closing_a_window_asks_its_host_and_a_close_by_the_host_reaches_the_app() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let open = create_signal(true);
    let held = StoredCell::new(None::<WindowHandle>);
    let heard = create_signal(Vec::<WindowReport>::new());
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(120.0, 80.0)}
                open={open}
                on_report={move |report: WindowReport| heard.update(|heard| heard.push(report))}
            >|handle: WindowHandle| {
                held.set(Some(handle));
                view! {
                    <Frame />
                }
            }</Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.take_window_requests();
    let handle = held.get().expect("the window built its content");
    assert!(
        handle.is_window(),
        "the content can tell it is a window of its own"
    );

    let (moved, resized) = with_installed(harness.document_mut(), |_| {
        (
            handle.begin_move(),
            handle.begin_resize(WindowEdge::BottomRight),
        )
    });
    assert!(
        moved && resized,
        "the host takes a move and a resize of a window it shows"
    );
    harness.frame(Vec::new());
    assert_eq!(
        harness.take_window_requests(),
        [
            WindowRequest::BeginMove(window),
            WindowRequest::BeginResize {
                window,
                edge: WindowEdge::BottomRight
            },
        ]
    );

    open.set(false);
    harness.frame(Vec::new());
    assert_eq!(
        harness.take_window_requests(),
        [WindowRequest::Close(window)],
        "closing the window asks the host to close it"
    );
    assert!(harness.windows().window(window).is_none());

    open.set(true);
    harness.frame(Vec::new());
    assert!(
        matches!(
            harness.take_window_requests().as_slice(),
            [WindowRequest::Open { window: opened, .. }] if *opened == window
        ),
        "opening it again asks again"
    );
    harness.frame(Vec::new());
    heard.set(Vec::new());

    harness.report_window(WindowReport::Closed(window));
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert_eq!(
        heard.get_untracked(),
        [WindowReport::Closed(window)],
        "a window the host closed tells the app"
    );
    assert!(
        harness.take_window_requests().is_empty(),
        "and asks the host for nothing back"
    );
    assert!(output.window(window).is_none(), "nor paints any longer");
}
