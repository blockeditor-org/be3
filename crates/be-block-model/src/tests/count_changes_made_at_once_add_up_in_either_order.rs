use super::*;

#[test]
fn count_changes_made_at_once_add_up_in_either_order() {
    let base = edited(&board(), [Board::VOTES.add(ObjectId::ROOT, 5)]);
    let ours = Board::VOTES.add(ObjectId::ROOT, 2);
    let theirs = Board::VOTES.add(ObjectId::ROOT, -7);

    let ours_first = edited(&base, [ours.clone(), theirs.clone()]);
    let theirs_first = edited(&base, [theirs, ours]);

    assert_eq!(ours_first.root().votes, Count(0));
    assert_eq!(ours_first, theirs_first);
}
