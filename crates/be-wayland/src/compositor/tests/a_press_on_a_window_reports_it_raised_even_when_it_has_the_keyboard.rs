use super::*;

fn press(harness: &mut Harness, at: Pos2) {
    harness.frame(vec![
        Input::PointerMoved(at),
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
}

#[test]
fn a_press_on_a_window_reports_it_raised_even_when_it_has_the_keyboard() {
    let mut harness = Harness::new();
    let (_first, first) = harness.open();
    let (_second, second) = harness.open();
    let windows = harness.compositor().windows();
    assert_eq!(
        windows.focused(),
        Some(second),
        "the window opened last has the keyboard"
    );

    let center = harness.window(first).center();
    press(&mut harness, center);
    assert_eq!(
        windows.focused(),
        Some(first),
        "the press gave the window the keyboard"
    );
    assert!(
        harness.output.as_ref().is_some_and(|output| output.repaint),
        "the frame that changed the list asks for another, so the host hears of it"
    );
    harness.settle();
    let raised = windows.raised(first);
    assert!(raised > 0, "the press brought the window forward");

    let revision = windows.revision();
    press(&mut harness, center);
    harness.settle();
    assert_eq!(windows.focused(), Some(first));
    assert_eq!(
        windows.raised(first),
        raised + 1,
        "a press on the window that already has the keyboard still brings it forward"
    );
    assert!(windows.revision() > revision, "and the host hears of it");
}
