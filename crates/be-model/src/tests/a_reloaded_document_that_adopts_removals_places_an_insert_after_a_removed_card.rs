use super::*;

#[test]
fn a_reloaded_document_that_adopts_removals_places_an_insert_after_a_removed_card() {
    let base = board();
    let (todo, _, write) = ids(&base);
    let removed = edited(&base, [Change::remove(write)]);
    let mut reloaded =
        Document::<Board>::from_bytes(&removed.to_bytes()).expect("the bytes decode");
    let (_, insert) = Column::CARDS.insert(todo, Anchor::After(write), &card("draft"));

    reloaded
        .adopt_session_state(&removed.session_state())
        .expect("the removals decode");
    reloaded.apply(&insert.into());

    assert_eq!(
        columns(&reloaded),
        owned(&[("Todo", &["draft", "review"]), ("Done", &[])])
    );
}
