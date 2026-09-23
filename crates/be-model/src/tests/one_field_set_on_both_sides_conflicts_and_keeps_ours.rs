use super::*;

#[test]
fn one_field_set_on_both_sides_conflicts_and_keeps_ours() {
    let base = board();
    let (_, _, write) = ids(&base);
    let ours = edited(&base, [Card::TEXT.set(write, &"ours".to_owned())]);
    let theirs = edited(
        &base,
        [
            Card::TEXT.set(write, &"theirs".to_owned()),
            Card::DONE.set(write, &true),
        ],
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(
        merged.read::<Card>(write),
        Some(Card {
            text: "ours".to_owned(),
            done: true,
        })
    );
}
