use super::*;

#[test]
fn cards_inserted_at_the_start_and_the_end_at_once_keep_their_ends() {
    let base = board();
    let (todo, _, _) = ids(&base);
    let (_, first) = Column::CARDS.insert(todo, Anchor::Start, &card("first"));
    let (_, last) = Column::CARDS.insert(todo, Anchor::End, &card("last"));

    let expected = owned(&[
        ("Todo", &["first", "write", "review", "last"]),
        ("Done", &[]),
    ]);
    assert_eq!(
        columns(&edited(&base, [first.clone(), last.clone()])),
        expected
    );
    assert_eq!(columns(&edited(&base, [last, first])), expected);
}
