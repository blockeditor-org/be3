use super::*;
use crate::Dismiss;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::primitives::*;
use crate::reactive::*;
use crate::{Window, WindowHandle, WindowId, WindowReport};

#[test]
fn a_menu_filling_a_window_fills_it_again_when_its_host_resizes_it() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let filled = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(120.0, 80.0)}>|_: WindowHandle|
                <Frame width=20.0 height=10.0>
                <Overlay
                    anchor=OverlayAnchor::Point(Pos2::ZERO)
                    placement=Placement::Fill
                    open=true
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame @node_ref=&filled />
                </Overlay>
                </Frame>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().node_rect(filled.get()),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(120.0, 80.0))),
        "the menu fills the window it opened in"
    );

    harness.report_window(WindowReport::Resized {
        window,
        size: vec2(200.0, 150.0),
    });
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().node_rect(filled.get()),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        "the host's resize alone places it again, with no input to lay it out"
    );
}
