use super::*;

#[test]
fn undoing_a_replace_swaps_back_and_redo_swaps_again() {
    let mut sequence = loaded("- [ ] a");
    let check = sequence
        .replace(ALICE, 3..4, b"x".to_vec())
        .expect("there is something to replace");
    let (undo, redo) = sequence
        .inverse(&check)
        .expect("the replace changes something");

    applied(&mut sequence, &check);
    applied(&mut sequence, &undo);
    assert_eq!(text(&sequence), "- [ ] a");
    assert_eq!(
        sequence.pos(3),
        Some(Pos {
            client: LOADED,
            offset: 3
        })
    );

    applied(&mut sequence, &redo);
    assert_eq!(text(&sequence), "- [x] a");
    applied(&mut sequence, &undo);

    let bob = sequence
        .replace(BOB, 3..4, b"x".to_vec())
        .expect("there is something to replace");
    applied(&mut sequence, &bob);
    assert_eq!(sequence.apply(&redo), None);
    assert_eq!(text(&sequence), "- [x] a");
}
