use super::*;
use crate::reactive::*;
use crate::unstyled::PanZoomApi;

#[test]
fn pinching_a_pan_zoom_with_two_fingers_zooms_and_pans_it() {
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let item = harness.find("item");
    let corner = pos2(300.0, 200.0);

    harness.finger(1, TouchPhase::Start, pos2(250.0, 200.0));
    harness.finger(2, TouchPhase::Start, pos2(350.0, 200.0));
    harness.move_fingers(pos2(200.0, 200.0), pos2(400.0, 200.0));
    harness.frame(Vec::new());

    let view = harness
        .document()
        .api::<PanZoomApi>(harness.find("stage"))
        .view()
        .get();
    assert!(
        (view.scale - 2.0).abs() < 0.01,
        "the view reached {}",
        view.scale
    );
    assert!(
        harness.rect(item).max.distance(corner) < 0.01,
        "the corner between the fingers moved to {:?}",
        harness.rect(item).max
    );

    harness.move_fingers(pos2(240.0, 200.0), pos2(440.0, 200.0));
    harness.frame(Vec::new());

    assert!(
        (harness
            .document()
            .api::<PanZoomApi>(harness.find("stage"))
            .view()
            .get()
            .scale
            - 2.0)
            .abs()
            < 0.01
    );
    assert!(
        harness.rect(item).max.distance(pos2(340.0, 200.0)) < 0.01,
        "the two finger pan moved the corner to {:?}",
        harness.rect(item).max
    );
}
