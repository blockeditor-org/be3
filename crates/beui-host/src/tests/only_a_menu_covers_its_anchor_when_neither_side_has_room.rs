use super::*;
use crate::Dismiss;
use crate::base::overlay::OverlayNode;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

fn placed(placement: Placement, height: f32) -> (f32, f32) {
    let _root = crate::reactive::RootScope::new();
    let overlay = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Frame>
                <Overlay
                    @node_ref=&overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 200.0))
                    placement=placement
                    open=true
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame width=40.0 height=height />
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 300.0));
    harness.frame(Vec::new());

    let overlay = kind_of::<OverlayNode>(harness.document(), overlay.get());
    let content = harness
        .document()
        .overlay_content(overlay)
        .expect("an open overlay has content");
    let rect = harness.rect(content);
    (rect.top(), rect.bottom())
}

#[test]
fn only_a_menu_covers_its_anchor_when_neither_side_has_room() {
    assert_eq!(
        placed(Placement::Menu, 250.0),
        (50.0, 300.0),
        "with 200 above its anchor and 100 below, a menu covers the anchor, as low as the window allows"
    );
    assert_eq!(
        placed(Placement::Menu, 1000.0),
        (0.0, 300.0),
        "a menu taller than the window is cut to the window's height"
    );
    assert_eq!(
        placed(Placement::BelowStart, 250.0),
        (0.0, 200.0),
        "anything else anchored below (a completion, a tooltip) takes the roomier side and ends at its anchor"
    );
    let (top, bottom) = placed(
        Placement::Popover {
            gap: 4,
            margin: 8,
        },
        250.0,
    );
    assert!(
        bottom <= 200.0 || top >= 200.0,
        "a popover keeps clear of its anchor too: {top}..{bottom}"
    );
}
