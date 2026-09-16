use super::*;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn pinching_a_pan_zoom_zooms_around_the_pointer() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let corner = pos2(300.0, 200.0);
    harness.pinch(corner, 1.5);
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert_eq!(view.scale, 1.5);
    let item = harness.rect(harness.find("item"));
    assert!(
        item.max.distance(corner) < 0.01,
        "the corner under the pointer moved to {:?}",
        item.max
    );
    assert_eq!(item.size(), Vec2::new(150.0, 75.0));
}
