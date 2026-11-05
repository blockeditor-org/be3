use super::*;

#[test]
fn moving_one_object_in_a_large_list_changes_two_parts() {
    let mut document = deck(3000);
    let before = document.to_parts();

    document.apply(
        &Deck::CARDS
            .move_into(ObjectId::ROOT, Anchor::After(numbered(2500)), numbered(100))
            .into(),
    );

    let parts = document.to_parts();
    assert_eq!(changed_parts(&before, &parts), 2);
    assert_eq!(from_parts::<Deck>(&parts), document);
}
