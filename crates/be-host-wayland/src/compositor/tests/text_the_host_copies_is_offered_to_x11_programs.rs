use super::*;

#[test]
fn text_the_host_copies_is_offered_to_x11_programs() {
    let mut harness = Harness::new();
    harness.settle();

    copy_in_the_host(&mut harness, "copied in the host");
    harness.settle();

    let offered = harness
        .compositor()
        .server
        .state
        .xwayland
        .offered(SelectionTarget::Clipboard)
        .map(<[String]>::to_vec)
        .expect("X11 is offered the host's text");
    assert!(offered.contains(&"UTF8_STRING".to_owned()));
    assert!(offered.contains(&"STRING".to_owned()));
    let pasted = paste_in_x11(
        &mut harness,
        SelectionTarget::Clipboard,
        "text/plain;charset=utf-8",
    );
    assert_eq!(pasted, "copied in the host");
}
