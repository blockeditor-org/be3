use super::*;

#[test]
fn it_keeps_on_enter_and_reverts_on_escape() {
    let _root = RootScope::new();
    let open = create_signal(true);
    let outcome = create_signal("");
    let mut test = DocumentTest::new(prompt(open, outcome), WIDE);
    press(&mut test, Key::Enter);
    assert!(!test.shows("keep-changes"), "Enter closes the prompt");
    assert_eq!(outcome.get_untracked(), "kept", "Enter keeps");

    open.set(true);
    test.frame(Vec::new());
    assert!(test.shows("keep-changes"), "the prompt opens again");
    press(&mut test, Key::Escape);
    assert!(!test.shows("keep-changes"), "Escape closes the prompt");
    assert_eq!(outcome.get_untracked(), "reverted", "Escape reverts");
}
