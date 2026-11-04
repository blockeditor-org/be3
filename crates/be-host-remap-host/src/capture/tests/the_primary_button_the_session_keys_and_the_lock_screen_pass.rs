use super::*;

#[test]
fn the_primary_button_the_session_keys_and_the_lock_screen_pass() {
    let mut capture = started();
    let click = [press("MouseLeft"), release("MouseLeft")];
    assert_eq!(caught(&mut capture, &click), click.to_vec());
    assert!(capture.catching(), "a click on Cancel is no chord");

    assert_eq!(caught(&mut capture, &[press("Ctrl"), press("Alt")]), vec![]);
    assert_eq!(
        caught(&mut capture, &[press("F2")]),
        vec![press("Ctrl"), press("Alt"), press("F2")],
        "the session switches terminals"
    );
    assert_eq!(capture.chord(), None);
    assert_eq!(
        caught(
            &mut capture,
            &[release("F2"), release("Alt"), release("Ctrl")]
        ),
        vec![release("F2"), release("Alt"), release("Ctrl")]
    );

    capture.set_locked(true);
    let typed = [press("Ctrl"), press("A"), release("A"), release("Ctrl")];
    assert_eq!(
        caught(&mut capture, &typed),
        typed.to_vec(),
        "the lock screen gets the password"
    );
    capture.set_locked(false);
    caught(&mut capture, &[press("Ctrl"), press("A")]);
    assert_eq!(capture.chord(), Some(&chord(&["Ctrl"], "A")));
}
