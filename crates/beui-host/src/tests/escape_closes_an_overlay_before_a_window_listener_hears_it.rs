use super::*;
use crate::base::overlay::OverlayNode;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Dismiss, KeyAt, KeyPhase};
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn escape_closes_an_overlay_before_a_window_listener_hears_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let heard = create_signal(0);
    let overlay = NodeRef::new();
    let document = build_node(move || {
        on_key(KeyAt::Window, KeyPhase::Bubble, move |event| {
            let escaped = event
                .press()
                .is_some_and(|press| press.pressed && press.key == Key::Escape);
            if escaped {
                heard.update(|heard| *heard += 1);
            }
            escaped
        });
        view! {
            <Frame>
                <Overlay
                    @node_ref=&overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open={open}
                    on_dismiss={move |_: Dismiss| open.set(false)}
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let overlay = kind_of::<OverlayNode>(harness.document(), overlay.get());
    assert!(harness.document().is_overlay_open(overlay.id()));

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(
        !harness.document().is_overlay_open(overlay.id()),
        "Escape closes the overlay"
    );
    assert_eq!(heard.get_untracked(), 0, "and goes no further");

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        heard.get_untracked(),
        1,
        "with nothing to close, the window hears it"
    );
}
