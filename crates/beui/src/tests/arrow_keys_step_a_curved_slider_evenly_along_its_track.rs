use super::*;
use crate::reactive::view;
use crate::styled::Slider;
use crate::unstyled::SliderScale;

#[test]
fn arrow_keys_step_a_curved_slider_evenly_along_its_track() {
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.0 min=0.0 max=120.0 scale={SliderScale::Midpoint(12.0)} />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    for _ in 0..10 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }
    let halfway = styled::slider_value(harness.document(), slider);
    assert!((halfway - 12.0).abs() < 0.5, "ten steps read {halfway}");

    for _ in 0..10 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }
    let end = styled::slider_value(harness.document(), slider);
    assert!((end - 120.0).abs() < 0.5, "twenty steps read {end}");
    assert!(
        end - halfway > halfway,
        "the steps above the midpoint were not the larger ones"
    );
}
