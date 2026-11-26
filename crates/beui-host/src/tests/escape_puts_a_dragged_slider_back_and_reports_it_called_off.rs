use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_puts_a_dragged_slider_back_and_reports_it_called_off() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(0.2);
    let drags = create_signal(Vec::new());
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider
                value={value}
                on_drag_change={move |change| drags.update(|drags| drags.push(change))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let track = harness.rect(slider);
    let travel = track.width() - 16.0;
    let thumb = pos2(track.left() + 8.0 + 0.2 * travel, track.center().y);

    harness.down(thumb, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(thumb + vec2(2.0, 0.0))]);
    harness.frame(Vec::new());
    assert!(
        (value.get_untracked() - (0.2 + 2.0 / travel)).abs() < 0.005,
        "the thumb follows the mouse from its first point, short of the drag threshold: {}",
        value.get_untracked()
    );
    harness.frame(vec![Input::PointerMoved(thumb + vec2(0.3 * travel, 0.0))]);
    harness.frame(Vec::new());
    assert!(
        (value.get_untracked() - 0.5).abs() < 0.01,
        "and on as it moves"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![Input::PointerMoved(thumb + vec2(0.5 * travel, 0.0))]);
    harness.up(thumb + vec2(0.5 * travel, 0.0), PointerButton::Primary);
    harness.frame(Vec::new());

    assert_eq!(
        value.get_untracked(),
        0.2,
        "Escape writes back where the drag started, and the moves and release after it leave it there"
    );
    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Cancelled],
        "the owner hears the drag called off, once"
    );
}
