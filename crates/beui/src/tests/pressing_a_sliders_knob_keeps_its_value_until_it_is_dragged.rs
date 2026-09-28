use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn pressing_a_sliders_knob_keeps_its_value_until_it_is_dragged() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.3 on_change={move |value| changed.borrow_mut().push(value)} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let track = harness.rect(slider);
    let knob = track.left() + 8.0 + 0.3 * (track.width() - 16.0);

    harness.click(pos2(knob + 5.0, track.center().y));
    assert!(
        changes.borrow().is_empty(),
        "pressing the knob does not move it"
    );

    harness.drag(
        pos2(knob + 5.0, track.center().y),
        pos2(knob + 5.0 + 0.2 * (track.width() - 16.0), track.center().y),
    );
    let last = *changes.borrow().last().expect("dragging the knob moves it");
    assert!((last - 0.5).abs() < 0.01, "{last}");

    harness.click(pos2(track.left() + 8.0, track.center().y));
    assert_eq!(
        changes.borrow().last(),
        Some(&0.0),
        "pressing the track moves the knob there"
    );
}
