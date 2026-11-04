use super::*;
use crate::reactive::*;
use crate::unstyled::{MAX_SCALE, MIN_SCALE, PanZoomApi};

#[test]
fn zooming_a_pan_zoom_stops_at_its_scale_limits() {
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let stage = harness.find("stage");

    harness.zoom(pos2(200.0, 150.0), 1_000.0);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<PanZoomApi>(stage)
            .view()
            .get()
            .scale,
        MAX_SCALE
    );

    harness.zoom(pos2(200.0, 150.0), 0.000_001);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<PanZoomApi>(stage)
            .view()
            .get()
            .scale,
        MIN_SCALE
    );
}
