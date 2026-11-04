use super::*;

#[test]
fn the_first_release_ends_a_chord_of_what_was_pressed() {
    let mut capture = started();
    assert_eq!(
        caught(&mut capture, &[press("Ctrl"), release("Ctrl")]),
        vec![]
    );
    assert_eq!(capture.chord(), Some(&chord(&[], "Ctrl")), "Ctrl alone");

    let mut capture = started();
    caught(&mut capture, &[press("F20")]);
    assert_eq!(capture.chord(), None, "a key alone waits: it may be held");
    caught(&mut capture, &[release("F20")]);
    assert_eq!(capture.chord(), Some(&chord(&[], "F20")));

    let mut capture = started();
    caught(
        &mut capture,
        &[
            press("Ctrl"),
            press("Shift"),
            release("Ctrl"),
            release("Shift"),
        ],
    );
    assert_eq!(
        capture.chord(),
        Some(&chord(&["Ctrl"], "Shift")),
        "the last pressed is the trigger, whichever is released"
    );
}
