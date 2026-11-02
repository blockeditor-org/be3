use super::*;

#[test]
fn a_move_takes_text_inserted_inside_it_along() {
    let mut sequence = loaded("one two three");
    let moved = sequence
        .move_range(4..8, 0)
        .expect("the range moves somewhere else");
    let insert = sequence
        .insert(BOB, 5, b"X".to_vec())
        .expect("there is something to insert");

    applied(&mut sequence, &insert);
    applied(&mut sequence, &moved);

    assert_eq!(text(&sequence), "tXwo one three");
}
