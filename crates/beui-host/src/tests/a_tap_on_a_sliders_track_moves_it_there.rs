use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_tap_on_a_sliders_track_moves_it_there() {
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
    let at = pos2(
        track.left() + 8.0 + 0.8 * (track.width() - 16.0),
        track.center().y,
    );

    harness.frame(vec![Input::PointerMoved(at)]);
    harness.frame(vec![
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
        },
    ]);
    let moved = changes.get_untracked();
    assert!(
        matches!(moved[..], [value] if (value - 0.8).abs() < 0.01),
        "a press and release that land in one frame, as a touchpad tap does, still move the knob once: {moved:?}"
    );
}
