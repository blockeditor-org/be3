use super::*;
use crate::LayerPolicy;
use crate::primitives::*;
use crate::{Window, WindowHandle, WindowId};
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn a_layer_that_traps_the_focus_in_a_window_takes_the_keyboard_there_with_nothing_focused() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(300.0, 200.0)}>|_: WindowHandle|
                <Frame width=300.0 height=200.0>
                    <Overlay
                        anchor=OverlayAnchor::Point(pos2(100.0, 100.0))
                        placement=Placement::At
                        policy=LayerPolicy::Pass
                        open={open}
                    >
                        <Frame width=40.0 height=30.0 />
                    </Overlay>
                </Frame>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(harness.context().keyboard_window(), WindowId::MAIN);

    open.set(true);
    let wants = harness
        .frame(Vec::new())
        .main()
        .window
        .requests
        .wants_keyboard;
    assert!(wants, "a layer that traps the focus wants the keyboard");
    assert_eq!(harness.document().focused_node(), None);
    assert_eq!(
        harness.context().keyboard_window(),
        window,
        "with nothing focused, the keyboard goes to the window of the layer that traps it"
    );
}
