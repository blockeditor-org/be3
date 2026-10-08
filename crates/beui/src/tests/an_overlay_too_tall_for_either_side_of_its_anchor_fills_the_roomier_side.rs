use super::*;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::reactive::{Frame, NodeRef, Overlay, build, view};

#[test]
fn an_overlay_too_tall_for_either_side_of_its_anchor_fills_the_roomier_side() {
    let overlay = NodeRef::new();
    let document = build({
        let overlay = overlay.clone();
        move || {
            view! {
                <Frame>
                    <Overlay
                        @node_ref=&overlay
                        anchor=OverlayAnchor::Point(pos2(10.0, 200.0))
                        placement=Placement::BelowStart
                        open=true
                        on_dismiss={|| {}}
                    >
                        <Frame width=40.0 height=1000.0 />
                    </Overlay>
                </Frame>
            }
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
    assert_eq!(
        (rect.top(), rect.bottom()),
        (0.0, 200.0),
        "the content takes the 200 above its anchor over the 100 below it, ending at the anchor"
    );
}
