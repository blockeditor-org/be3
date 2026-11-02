use super::*;

#[test]
fn a_peer_edit_under_local_edits_touches_only_what_either_changed() {
    let mut confirmed = board();
    let (todo, done, write) = ids(&confirmed);
    let review = card_id(&confirmed, "review");
    let mut visible = confirmed.clone();
    let local = vec![
        Edit::from(Card::TEXT.set(write, &"draft".to_owned())),
        Edit::from(Column::CARDS.move_into(done, Anchor::End, write)),
    ];
    for edit in &local {
        visible.apply(edit);
    }

    let incoming = Edit::from(Card::DONE.set(review, &true));
    let mut touched = Vec::new();
    visible.rebase(&mut confirmed, &incoming, &local, &mut touched);

    let mut rebuilt = confirmed.clone();
    for edit in &local {
        rebuilt.apply(edit);
    }
    assert_eq!(visible, rebuilt);
    assert_eq!(
        columns(&visible),
        owned(&[("Todo", &["review"]), ("Done", &["draft"])])
    );
    assert!(visible.field(review, Card::DONE));
    assert!(!touched.contains(&Touched::Everything));
    assert!(touched.contains(&Touched::Field(review, Card::DONE.index())));
    assert!(touched.contains(&Touched::Field(todo, Column::CARDS.index())));
    assert!(!touched.contains(&Touched::Field(ObjectId::ROOT, Board::TITLE.index())));
    assert!(!touched.contains(&Touched::Field(review, Card::TEXT.index())));
}
