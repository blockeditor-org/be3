use super::*;

#[test]
fn counts_merge_by_adding_both_sides() {
    let base = edited(&board(), [Board::VOTES.add(ObjectId::ROOT, 10)]);
    let ours = edited(&base, [Board::VOTES.add(ObjectId::ROOT, 2)]);
    let theirs = edited(&base, [Board::VOTES.add(ObjectId::ROOT, -4)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().votes, Count(8));
}
