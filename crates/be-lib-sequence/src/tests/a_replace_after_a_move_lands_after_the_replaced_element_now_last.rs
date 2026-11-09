use super::*;

#[test]
fn a_replace_after_a_move_lands_after_the_replaced_element_now_last() {
    let mut sequence = loaded("abcdef");
    let replace = sequence
        .replace(ALICE, 0..2, b"X".to_vec())
        .expect("there is something to replace");
    let moved = sequence
        .move_range(0..1, 6)
        .expect("the element moves somewhere else");

    applied(&mut sequence, &moved);
    assert_eq!(text(&sequence), "bcdefa");
    applied(&mut sequence, &replace);

    assert_eq!(
        text(&sequence),
        "cdefX",
        "the insert follows the moved `a`, last of what it replaced, not `b`, last as listed"
    );
}
