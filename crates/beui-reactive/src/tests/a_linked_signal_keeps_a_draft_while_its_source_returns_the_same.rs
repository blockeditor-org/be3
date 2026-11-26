use super::*;

#[test]
fn a_linked_signal_keeps_a_draft_while_its_source_returns_the_same() {
    let _root = RootScope::new();
    let saved = create_signal(("name".to_owned(), 1));
    let draft = create_linked_signal(move || saved.with(|(name, _)| name.clone()));
    draft.set("typed".to_owned());
    saved.update(|(_, revision)| *revision += 1);
    flush();
    assert_eq!(
        draft.get_untracked(),
        "typed",
        "an edit beside the name keeps the draft"
    );
    saved.update(|(name, _)| *name = "renamed".to_owned());
    flush();
    assert_eq!(draft.get_untracked(), "renamed");
}
