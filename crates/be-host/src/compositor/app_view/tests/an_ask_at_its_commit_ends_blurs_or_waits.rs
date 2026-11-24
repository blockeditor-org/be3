use super::*;

#[test]
fn an_ask_at_its_commit_ends_blurs_or_waits() {
    // (wanting, up, within, own)
    assert_eq!(
        ask_at_commit(true, true, true, true),
        AskAtCommit::Ends,
        "a popover or other keyboard-taking view is open: the ask does nothing"
    );
    assert_eq!(
        ask_at_commit(false, false, false, false),
        AskAtCommit::Ends,
        "the keyboard is outside the editor: the ask never takes it from there"
    );
    assert_eq!(
        ask_at_commit(false, false, true, true),
        AskAtCommit::Blurs,
        "the child is not up: the keyboard leaves the editor's own view at once"
    );
    assert_eq!(
        ask_at_commit(false, false, true, false),
        AskAtCommit::Waits,
        "a keyboard in another of the editor's children stays there until the child is up"
    );
    assert_eq!(
        ask_at_commit(false, true, true, true),
        AskAtCommit::Waits,
        "a child up takes it in its view, at once"
    );
}
