use super::*;

#[test]
fn a_field_changed_on_one_side_merges_to_that_change_from_either_side() {
    let base = board();
    let changed = edited(
        &base,
        [Board::TITLE.set(ObjectId::ROOT, &"Launch".to_owned())],
    );

    let (ours_changed, ours_conflicts) = Document::merge(&base, &changed, &base);
    let (theirs_changed, theirs_conflicts) = Document::merge(&base, &base, &changed);

    assert_eq!((ours_conflicts, theirs_conflicts), (0, 0));
    assert_eq!(ours_changed.root().title, "Launch");
    assert_eq!(theirs_changed.root().title, "Launch");
}
