use super::*;

#[test]
fn bytes_that_are_not_utf8_are_kept_as_they_are() {
    let mut sequence = Sequence::from_items(vec![0xff, 0xfe]);
    let insert = sequence
        .insert(ALICE, 1, vec![0xc3])
        .expect("there is something to insert");

    applied(&mut sequence, &insert);

    assert_eq!(sequence.items(), [0xff, 0xc3, 0xfe]);
    assert!(text(&sequence).contains('\u{fffd}'));
}
