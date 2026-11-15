use super::*;

#[test]
fn a_linked_signal_holds_writes_until_its_source_changes() {
    let _root = RootScope::new();
    let saved = create_signal("first".to_owned());
    let draft = create_linked_signal(move || saved.get());
    assert_eq!(draft.get_untracked(), "first", "it starts from its source");
    draft.set("edited".to_owned());
    flush();
    assert_eq!(
        draft.get_untracked(),
        "edited",
        "a write holds while the source is still"
    );
    saved.set("second".to_owned());
    flush();
    assert_eq!(
        draft.get_untracked(),
        "second",
        "a change to the source resets it"
    );
}
