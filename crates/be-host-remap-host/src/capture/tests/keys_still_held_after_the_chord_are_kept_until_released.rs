use super::*;

#[test]
fn keys_still_held_after_the_chord_are_kept_until_released() {
    let mut capture = started();
    caught(&mut capture, &[press("Ctrl"), press("C")]);
    assert!(capture.chord().is_some());
    assert_eq!(
        caught(&mut capture, &[release("C"), release("Ctrl")]),
        vec![],
        "programs never saw them pressed"
    );
    assert_eq!(
        caught(&mut capture, &[press("C"), release("C")]),
        vec![press("C"), release("C")],
        "nothing is caught until it starts again"
    );
    assert_eq!(capture.chord(), Some(&chord(&["Ctrl"], "C")));

    let mut capture = started();
    caught(&mut capture, &[press("Shift")]);
    capture.stop();
    assert_eq!(
        caught(&mut capture, &[press("A"), release("Shift"), release("A")]),
        vec![press("A"), release("A")],
        "closed mid-capture, it keeps only the releases of what it kept"
    );
    assert_eq!(capture.chord(), None);
}
