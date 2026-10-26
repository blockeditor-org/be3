use super::*;

#[test]
fn it_reverts_once_its_countdown_runs_out() {
    let _root = RootScope::new();
    let open = create_signal(true);
    let outcome = create_signal("");
    let mut test = DocumentTest::new(prompt(open, outcome), WIDE);
    assert!(test.shows("keep-changes.confirm"), "the prompt opens");
    assert!(test.shows("keep-changes.countdown"));

    test.advance(Duration::from_secs(13));
    test.frame(Vec::new());
    assert!(
        test.shows("keep-changes"),
        "the prompt waits out the countdown"
    );
    assert_eq!(outcome.get_untracked(), "", "nothing is decided yet");

    test.advance(Duration::from_secs(2));
    test.frame(Vec::new());
    test.frame(Vec::new());
    assert!(!test.shows("keep-changes"), "the prompt closes at zero");
    assert_eq!(outcome.get_untracked(), "reverted", "running out reverts");
}
