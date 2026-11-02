use super::*;

#[test]
fn undoing_a_move_puts_the_range_back() {
    let mut sequence = loaded("abcdef");
    let moved = sequence
        .move_range(0..2, 6)
        .expect("the range moves somewhere else");
    let (undo, redo) = sequence
        .inverse(&moved)
        .expect("the move changes something");

    applied(&mut sequence, &moved);
    assert_eq!(text(&sequence), "cdefab");
    applied(&mut sequence, &undo);
    assert_eq!(text(&sequence), "abcdef");
    applied(&mut sequence, &redo);
    assert_eq!(text(&sequence), "cdefab");
}
