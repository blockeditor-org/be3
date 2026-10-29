use super::*;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::primitives::*;
use crate::{Dismiss, LayerPolicy};

#[test]
fn a_passing_overlay_occludes_only_its_content() {
    let document = build_node(|| {
        view! {
            <Frame>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Pass
                    open=true
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let layers = harness.document().overlay_layers(crate::WindowId::MAIN);
    assert_eq!(layers.len(), 1);
    let (_, occluder) = layers[0];
    assert!(
        occluder.width() <= 40.0 && occluder.height() <= 30.0,
        "a passing layer lets the pointer through outside its content, so only the content occludes: {occluder:?}"
    );
}
