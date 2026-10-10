use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn a_tap_on_a_sliders_track_moves_it_there() {
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
    let at = pos2(
        track.left() + 8.0 + 0.8 * (track.width() - 16.0),
        track.center().y,
    );

    harness.frame(vec![Event::PointerMoved(at)]);
    harness.frame(vec![
        Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    let moved = changes.borrow().clone();
    assert!(
        matches!(moved[..], [value] if (value - 0.8).abs() < 0.01),
        "a press and release that land in one frame, as a touchpad tap does, still move the knob once: {moved:?}"
    );
}
