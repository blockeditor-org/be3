use super::*;
use crate::reactive::*;
use crate::unstyled::PanZoomApi;

#[test]
fn pinching_a_pan_zoom_zooms_around_the_pointer() {
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let corner = pos2(300.0, 200.0);
    harness.zoom(corner, 1.5);
    harness.frame(Vec::new());

    let view = harness
        .document()
        .api::<PanZoomApi>(harness.find("stage"))
        .view()
        .get();
    assert!((view.scale - 1.5).abs() < 1e-5, "{}", view.scale);
    let item = harness.rect(harness.find("item"));
    assert!(
        item.max.distance(corner) < 0.01,
        "the corner under the pointer moved to {:?}",
        item.max
    );
    assert!(
        (item.size() - Vec2::new(150.0, 75.0)).length() < 0.01,
        "{:?}",
        item.size()
    );
}
