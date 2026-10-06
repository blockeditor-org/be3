use super::*;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::reactive::{Frame, Overlay, build, view};

#[test]
fn a_light_overlay_occludes_only_its_content() {
    let document = build(|| {
        view! {
            <Frame>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::BelowStart
                    light=true
                    open=true
                    on_dismiss={|| {}}
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let layers = harness.document().overlay_layers();
    assert_eq!(layers.len(), 1);
    let (_, occluder) = layers[0];
    assert!(
        occluder.width() <= 40.0 && occluder.height() <= 30.0,
        "a light overlay lets the pointer through outside its content, so only the content occludes: {occluder:?}"
    );
}
