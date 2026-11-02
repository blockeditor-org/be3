use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Extent, WindowId};

#[test]
fn a_window_sized_to_its_content_lays_out_once_at_that_size() {
    let _root = crate::reactive::RootScope::new();
    let wide = create_signal(120.0_f32);
    let row = NodeRef::new();
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(500.0, 500.0)}
                fit={Some(vec2(300.0, 400.0))}
            >|_: WindowHandle|
                <List spacing=0.0>
                    <Dynamic value={wide}>|wide: f32|
                        <Frame width=wide height=20.0 />
                    </Dynamic>
                    <Frame @node_ref=&row width={Extent::fraction(1.0)} height=20.0 />
                </List>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().fitted_window_size(window),
        Some(vec2(120.0, 40.0)),
        "the window takes its content's size, not the size it was given"
    );
    assert_eq!(
        harness.rect(row.get()).width(),
        120.0,
        "a row that fills its window fills the width the window took"
    );

    wide.set(200.0);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().fitted_window_size(window),
        Some(vec2(200.0, 40.0))
    );
    assert_eq!(harness.rect(row.get()).width(), 200.0);
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "the new size is worked out before the layout, which runs once at it"
    );

    wide.set(900.0);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().fitted_window_size(window),
        Some(vec2(300.0, 40.0)),
        "content wider than the window may grow is cut to the most it may take"
    );
    assert_eq!(harness.rect(row.get()).width(), 300.0);
}
