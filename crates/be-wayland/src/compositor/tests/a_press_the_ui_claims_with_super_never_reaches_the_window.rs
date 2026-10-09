use super::*;

fn button(pos: Pos2, button: PointerButton, pressed: bool, modifiers: beui::Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

#[test]
fn a_press_the_ui_claims_with_super_never_reaches_the_window() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let center = harness.window(id).center();
    harness.client.received.buttons.clear();

    let mut events = super_key(true);
    events.push(Event::PointerMoved(center));
    harness.frame(events);
    for which in [PointerButton::Primary, PointerButton::Secondary] {
        harness.frame(vec![button(center, which, true, beui::Modifiers::LOGO)]);
        harness.frame(vec![Event::PointerMoved(center + Vec2::new(30.0, 20.0))]);
        harness.frame(vec![button(
            center + Vec2::new(30.0, 20.0),
            which,
            false,
            beui::Modifiers::LOGO,
        )]);
    }
    harness.frame(vec![
        button(center, PointerButton::Primary, true, beui::Modifiers::LOGO),
        button(center, PointerButton::Primary, false, beui::Modifiers::LOGO),
    ]);
    harness.frame(super_key(false));
    harness.settle();
    assert!(
        harness.client.received.buttons.is_empty(),
        "no press made with Super held over the claimed area reached the program: {:?}",
        harness.client.received.buttons
    );
    assert_eq!(
        CLAIMED.with(std::cell::Cell::get),
        2,
        "the UI that claimed them heard both primary presses"
    );

    harness.click(&format!("wayland.window.{}", id.0));
    assert_eq!(
        harness.client.received.buttons,
        vec![(BUTTON_LEFT, true), (BUTTON_LEFT, false)],
        "a press without Super is the program's"
    );
}
