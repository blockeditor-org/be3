use super::*;

#[test]
fn editing_inserting_or_removing_one_object_in_a_large_list_changes_one_part() {
    let document = deck(3000);
    let before = document.to_parts();
    assert!(before.children().len() > 5, "the list splits into parts");

    let edited = edited_deck(&document, Card::TEXT.set(numbered(1500), &"edited".to_owned()));
    let parts = edited.to_parts();
    assert_eq!(changed_parts(&before, &parts), 1);
    assert_eq!(from_parts::<Deck>(&parts), edited);

    let id = (5000..)
        .map(numbered)
        .find(|id| crate::parts::never_ends_part(*id))
        .expect("some id doesn't end a part");
    let inserted = edited_deck(
        &document,
        Deck::CARDS.insert_as(id, ObjectId::ROOT, Anchor::After(numbered(1500)), &card("new")),
    );
    let parts = inserted.to_parts();
    assert_eq!(changed_parts(&before, &parts), 1);
    assert_eq!(from_parts::<Deck>(&parts), inserted);

    let removed = edited_deck(&document, Change::remove(numbered(1500)));
    let parts = removed.to_parts();
    assert_eq!(changed_parts(&before, &parts), 1);
    assert_eq!(from_parts::<Deck>(&parts), removed);
}

fn edited_deck(document: &Document<Deck>, change: Change) -> Document<Deck> {
    let mut document = document.clone();
    document.apply(&change.into());
    document
}
