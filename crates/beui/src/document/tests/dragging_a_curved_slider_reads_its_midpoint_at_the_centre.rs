use super::*;
use crate::reactive::view;
use crate::styled::Slider;
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
        harness.drag(pos2(track.left() + 1.0, middle.y), middle);
        harness.frame(Vec::new());
    }

    let curved_value = styled::slider_value(harness.document(), curved);
    let linear_value = styled::slider_value(harness.document(), linear);
    assert!(
        (curved_value - 12.0).abs() < 1.0,
        "the curved slider read {curved_value}"
    );
    assert!(
        (linear_value - 60.0).abs() < 1.0,
        "the linear slider read {linear_value}"
    );
}
