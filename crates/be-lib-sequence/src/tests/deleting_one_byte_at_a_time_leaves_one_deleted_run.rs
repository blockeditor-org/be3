use super::*;

#[test]
fn deleting_one_byte_at_a_time_leaves_one_deleted_run() {
    let mut sequence = loaded("abcdef");
    for at in [5, 4, 3] {
        let delete = sequence
            .delete(at..at + 1)
            .expect("there is a byte to delete");
        applied(&mut sequence, &delete);
    }
    assert_eq!(text(&sequence), "abc");
    assert_eq!(sequence.fragment_count(), 2);

    for at in [0, 0] {
        let delete = sequence
            .delete(at..at + 1)
            .expect("there is a byte to delete");
        applied(&mut sequence, &delete);
    }
    assert_eq!(text(&sequence), "c");
    assert_eq!(
        sequence.fragment_count(),
        3,
        "deleting forwards joins too, and the kept byte stays apart"
    );

    let last = sequence.delete(0..1).expect("there is a byte to delete");
    applied(&mut sequence, &last);
    assert_eq!(sequence.fragment_count(), 1);
}
