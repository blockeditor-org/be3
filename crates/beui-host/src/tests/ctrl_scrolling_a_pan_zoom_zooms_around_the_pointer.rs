use super::*;
use crate::geometry::vec2;
use crate::reactive::*;
use crate::unstyled::PanZoomApi;

#[test]
fn ctrl_scrolling_a_pan_zoom_zooms_around_the_pointer() {
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let corner = pos2(300.0, 200.0);
    harness.scroll_at(corner, vec2(0.0, 25.0), Modifiers::CTRL);
    harness.frame(Vec::new());

    let view = harness
        .document()
        .api::<PanZoomApi>(harness.find("stage"))
        .view()
        .get();
    assert!(view.scale > 1.05, "the view only reached {}", view.scale);
    let item = harness.rect(harness.find("item"));
    assert!(
        item.max.distance(corner) < 0.01,
        "the corner under the pointer moved to {:?}",
        item.max
    );
    assert_eq!(item.width(), item.width().round());
    assert!((item.width() - 100.0 * view.scale).abs() <= 1.0);
}
