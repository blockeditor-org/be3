use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn dragging_a_slider_moves_its_value() {
    let _root = crate::reactive::RootScope::new();
    let (reported, set_reported) = create_signal(0.0);
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.0 on_change={move |value| set_reported.set(value)} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let track = harness.rect(slider);
    let middle = track.center();
    harness.drag(pos2(track.left() + 8.0, middle.y), middle);
    harness.frame(Vec::new());

    let value = styled::slider_value(harness.document(), slider);
    assert!((value - 0.5).abs() < 0.01, "the slider read {value}");
    assert_eq!(reported.get_untracked(), value);
}
