use super::*;
use crate::reactive::*;
use crate::unstyled::PanZoomApi;

#[test]
fn plus_and_minus_zoom_a_focused_pan_zoom_and_zero_resets_the_scale() {
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let stage = harness.find("stage");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Plus, Modifiers::NONE);
    harness.frame(Vec::new());
    let zoomed = harness.document().api::<PanZoomApi>(stage).view().get();
    assert!(
        (zoomed.scale - 1.25).abs() < 0.001,
        "the view reached {}",
        zoomed.scale
    );
    assert_eq!(zoomed.center, pos2(0.0, 0.0));

    harness.key(Key::Minus, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        (harness
            .document()
            .api::<PanZoomApi>(stage)
            .view()
            .get()
            .scale
            - 1.0)
            .abs()
            < 0.001
    );

    harness.key(Key::Plus, Modifiers::NONE);
    harness.key(Key::Plus, Modifiers::NONE);
    harness.key(Key::Zero, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<PanZoomApi>(stage)
            .view()
            .get()
            .scale,
        1.0
    );
}
