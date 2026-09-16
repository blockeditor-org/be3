use super::*;
use crate::reactive::view;
use crate::unstyled::{MAX_SCALE, MIN_SCALE, PanZoomView, pan_zoom_view};

#[test]
fn zooming_a_pan_zoom_stops_at_its_scale_limits() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let stage = harness.find("stage");

    harness.pinch(pos2(200.0, 150.0), 1_000.0);
    harness.frame(Vec::new());
    assert_eq!(
        pan_zoom_view(harness.document(), stage).get().scale,
        MAX_SCALE
    );

    harness.pinch(pos2(200.0, 150.0), 0.000_001);
    harness.frame(Vec::new());
    assert_eq!(
        pan_zoom_view(harness.document(), stage).get().scale,
        MIN_SCALE
    );
}
