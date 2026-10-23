use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::SliderScale;

#[test]
fn dragging_a_curved_slider_reads_its_midpoint_at_the_centre() {
    let (document, [curved, linear]) = toolbar_of(|| {
        [
            view! {
                <Slider value=0.0 min=0.0 max=120.0 scale={SliderScale::Midpoint(12.0)} />
            },
            view! {
                <Slider value=0.0 min=0.0 max=120.0 />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    for slider in [curved, linear] {
        let track = harness.rect(slider);
        let middle = track.center();
        harness.drag(pos2(track.left() + 8.0, middle.y), middle);
        harness.frame(Vec::new());
    }

    let curved_value = harness
        .document()
        .api::<styled::SliderApi>(curved)
        .value()
        .get();
    let linear_value = harness
        .document()
        .api::<styled::SliderApi>(linear)
        .value()
        .get();
    assert!(
        (curved_value - 12.0).abs() < 1.0,
        "the curved slider read {curved_value}"
    );
    assert!(
        (linear_value - 60.0).abs() < 1.0,
        "the linear slider read {linear_value}"
    );
}
