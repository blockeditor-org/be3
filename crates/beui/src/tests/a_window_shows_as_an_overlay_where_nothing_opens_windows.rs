use super::*;
use crate::primitives::{Window, WindowHandle};

#[test]
fn a_window_shows_as_an_overlay_where_nothing_opens_windows() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let held = StoredCell::new(None::<WindowHandle>);
    let content = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Window size={vec2(120.0, 80.0)} at={pos2(50.0, 40.0)}>|handle: WindowHandle| {
                held.set(Some(handle));
                view! {
                    <Interactive on_click={move || clicks.update(|clicks| *clicks += 1)}>
                        <Frame @node_ref=&content fill={Color32::WHITE} />
                    </Interactive>
                }
            }</Window>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    assert!(
        output.window_requests.is_empty(),
        "a window asks nothing of a platform that opens no windows: {:?}",
        output.window_requests
    );
    assert_eq!(
        output.windows.len(),
        1,
        "everything paints into the main window"
    );
    assert_eq!(
        harness.document().node_rect(content.get()),
        Some(Rect::from_min_size(pos2(50.0, 40.0), vec2(120.0, 80.0))),
        "the window's content sits where an overlay at its place would, at its size"
    );
    let handle = held.get().expect("the window built its content");
    assert!(
        !with_installed(harness.document_mut(), |_| handle.is_window()),
        "the content can tell it is not a window of its own"
    );
    assert!(
        !with_installed(harness.document_mut(), |_| handle.begin_move()),
        "a move is left to the caller where the host cannot move the window"
    );

    harness.click(pos2(60.0, 50.0));
    assert_eq!(
        clicks.get_untracked(),
        1,
        "a press on the overlay reaches its content"
    );
}
