use super::*;

#[test]
fn keys_held_when_it_starts_pass_until_released() {
    let mut capture = Capture::default();
    assert_eq!(
        caught(&mut capture, &[press("Enter")]),
        vec![press("Enter")]
    );
    capture.start();
    assert_eq!(
        caught(&mut capture, &[release("Enter")]),
        vec![release("Enter")],
        "the key pressed before passes on, and makes no chord"
    );
    assert_eq!(capture.chord(), None);
    assert!(capture.catching());

    caught(&mut capture, &[press("Ctrl")]);
    capture.start();
    assert_eq!(
        capture.held(),
        [] as [Source; 0],
        "a new capture starts empty"
    );
    assert_eq!(
        caught(&mut capture, &[release("Ctrl")]),
        vec![],
        "a key kept by the capture before stays kept"
    );
    assert_eq!(capture.chord(), None);
}
