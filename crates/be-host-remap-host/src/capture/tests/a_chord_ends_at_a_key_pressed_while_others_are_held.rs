use super::*;

#[test]
fn a_chord_ends_at_a_key_pressed_while_others_are_held() {
    let mut capture = started();
    assert_eq!(caught(&mut capture, &[press("RightCtrl")]), vec![]);
    assert_eq!(capture.held(), [key("RightCtrl")]);
    assert_eq!(capture.chord(), None);

    assert_eq!(caught(&mut capture, &[press("C")]), vec![]);
    assert_eq!(capture.chord(), Some(&chord(&["RightCtrl"], "C")));
    assert!(!capture.catching(), "the chord ends the capture");
    assert_eq!(capture.held(), [] as [Source; 0]);

    let mut capture = started();
    caught(&mut capture, &[press("MouseSide"), press("MouseMiddle")]);
    assert_eq!(
        capture.chord(),
        Some(&chord(&["MouseSide"], "MouseMiddle")),
        "a held button counts as held too"
    );

    let mut capture = started();
    caught(
        &mut capture,
        &[press("LeftCtrl"), press("LeftShift"), press("RightAlt")],
    );
    assert_eq!(capture.chord(), None, "modifiers wait for more");
    assert_eq!(
        capture.held(),
        [key("LeftCtrl"), key("LeftShift"), key("RightAlt")]
    );
}
