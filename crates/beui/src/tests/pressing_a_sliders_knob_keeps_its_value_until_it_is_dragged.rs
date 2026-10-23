use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn pressing_a_sliders_knob_keeps_its_value_until_it_is_dragged() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider
                value={Binding::new(0.3, move |next| changes.update(|changes| changes.push(next)))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let track = harness.rect(slider);
    let knob = track.left() + 8.0 + 0.3 * (track.width() - 16.0);

    harness.click(pos2(knob + 5.0, track.center().y));
    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "pressing the knob does not move it"
    );

    harness.drag(
        pos2(knob + 5.0, track.center().y),
        pos2(knob + 5.0 + 0.2 * (track.width() - 16.0), track.center().y),
    );
    let last = *changes
        .get_untracked()
        .last()
        .expect("dragging the knob moves it");
    assert!((last - 0.5).abs() < 0.01, "{last}");

    harness.click(pos2(track.left() + 8.0, track.center().y));
    assert_eq!(
        changes.get_untracked().last(),
        Some(&0.0),
        "pressing the track moves the knob there"
    );
}
