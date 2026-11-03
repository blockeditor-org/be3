use super::*;
use crate::{Window, WindowHandle, WindowId};

#[test]
fn a_kept_alive_window_that_closes_needs_no_damage_where_it_painted() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let open = create_signal(true);
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(120.0, 80.0)}
                open={open}
                keep_alive={true}
            >|_: WindowHandle|
                <Frame fill={Color32::WHITE} />
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(
        output.window(window).is_some(),
        "the open window paints an output of its own"
    );

    open.set(false);
    let output = harness.frame(Vec::new());
    assert!(
        output.window(window).is_none(),
        "a closed window is no longer shown, so what it painted needs no damage, even though its content is kept"
    );

    open.set(true);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(
        output.window(window).is_some(),
        "and it paints again once it reopens"
    );
}
