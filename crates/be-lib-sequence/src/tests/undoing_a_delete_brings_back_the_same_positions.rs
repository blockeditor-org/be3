use super::*;

#[test]
fn undoing_a_delete_brings_back_the_same_positions() {
    let mut sequence = loaded("abc");
    let delete = sequence.delete(1..2).expect("there is something to delete");
    let (undo, _) = sequence
        .inverse(&delete)
        .expect("the delete changes something");
    let b = Pos {
        client: LOADED,
        offset: 1,
    };

    applied(&mut sequence, &delete);
    applied(
        &mut sequence,
        &SeqOp::Insert {
            after: Some(b),
            client: BOB,
            start: 0,
            items: b"X".to_vec(),
        },
    );
    assert_eq!(text(&sequence), "aXc");
    applied(&mut sequence, &undo);

    assert_eq!(text(&sequence), "abXc");
    assert_eq!(sequence.index(b), Some(1));
}
