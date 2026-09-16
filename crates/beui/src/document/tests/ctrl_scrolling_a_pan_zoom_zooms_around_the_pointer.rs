use super::*;
use crate::geometry::vec2;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn ctrl_scrolling_a_pan_zoom_zooms_around_the_pointer() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let corner = pos2(300.0, 200.0);
    harness.scroll(corner, vec2(0.0, 25.0), Modifiers::CTRL);
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert!(view.scale > 1.05, "the view only reached {}", view.scale);
    let item = harness.rect(harness.find("item"));
    assert!(
        item.max.distance(corner) < 0.01,
        "the corner under the pointer moved to {:?}",
        item.max
    );
    assert!((item.width() - 100.0 * view.scale).abs() < 0.01);
}
