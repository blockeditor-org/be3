use super::*;

#[test]
fn undo_and_redo_are_asked_of_the_host() {
    let document = BlockDocument::new(Waker::default());
    document.adopt(&TextBlock::of("text"));
    let revision = document.revision();

    assert_eq!(document.undo(), None);
    assert_eq!(document.take_history(), Some(History::Undo));
    assert!(
        document.revision() > revision,
        "the editor syncs, so the request reaches the pump"
    );
    assert_eq!(document.changes_since(revision), Some(TextChange::NONE));

    assert_eq!(document.redo(), None);
    assert_eq!(document.take_history(), Some(History::Redo));
    assert_eq!(document.take_history(), None);
}
