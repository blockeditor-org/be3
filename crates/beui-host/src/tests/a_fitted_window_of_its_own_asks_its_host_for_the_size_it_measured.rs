use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::{WindowId, WindowRequest};

#[test]
fn a_fitted_window_of_its_own_asks_its_host_for_the_size_it_measured() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let wide = create_signal(120.0_f32);
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(500.0, 500.0)}
                fit={Some(vec2(800.0, 600.0))}
            >|_: WindowHandle|
                <Dynamic value={wide}>|wide: f32|
                    <Frame width=wide height=40.0 />
                </Dynamic>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let opened = harness.take_window_requests();
    assert!(
        opened.iter().any(|request| matches!(
            request,
            WindowRequest::Open { window: opened, size, .. }
                if *opened == window && *size == vec2(120.0, 40.0)
        )),
        "it opens at the size it measured, not the size it was given: {opened:?}"
    );

    wide.set(300.0);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let resized = harness.take_window_requests();
    assert!(
        resized.contains(&WindowRequest::Resize {
            window,
            size: vec2(300.0, 40.0),
        }),
        "and asks again when what it holds grows: {resized:?}"
    );
}
