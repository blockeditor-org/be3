use super::*;
use crate::reactive::*;
use crate::styled::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Heard {
    Drag(ValueDrag),
    Value(f32),
}

#[test]
fn a_press_off_a_sliders_thumb_reports_the_drag_before_its_jump() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider
                value={Binding::new(0.2, move |next| heard.update(|heard| heard.push(Heard::Value(next))))}
                on_drag_change={move |change| heard.update(|heard| heard.push(Heard::Drag(change)))}
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

    harness.down(at, PointerButton::Primary);
    harness.up(at, PointerButton::Primary);
    harness.frame(Vec::new());

    let heard = heard.get_untracked();
    assert_eq!(
        heard.first(),
        Some(&Heard::Drag(ValueDrag::Started)),
        "the drag starts before the jump writes, so an owner holds the jump too: {heard:?}"
    );
    assert!(
        matches!(heard[1], Heard::Value(value) if (value - 0.8).abs() < 0.02),
        "{heard:?}"
    );
    assert_eq!(heard.last(), Some(&Heard::Drag(ValueDrag::Kept)));
}
