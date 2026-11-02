use super::*;

#[test]
fn counts_saturate_instead_of_wrapping() {
    let base = edited(&board(), [Board::VOTES.add(ObjectId::ROOT, i64::MAX - 1)]);
    let ours = edited(&base, [Board::VOTES.add(ObjectId::ROOT, 1)]);
    let theirs = edited(&base, [Board::VOTES.add(ObjectId::ROOT, 1)]);

    assert_eq!(
        edited(&ours, [Board::VOTES.add(ObjectId::ROOT, 5)])
            .root()
            .votes,
        Count(i64::MAX)
    );
    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().votes, Count(i64::MAX));
    assert_eq!(
        edited(
            &board(),
            [
                Board::VOTES.add(ObjectId::ROOT, i64::MIN),
                Board::VOTES.add(ObjectId::ROOT, -1)
            ]
        )
        .root()
        .votes,
        Count(i64::MIN)
    );
}
