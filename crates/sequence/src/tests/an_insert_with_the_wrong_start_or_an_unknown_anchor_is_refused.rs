use super::*;

#[test]
fn an_insert_with_the_wrong_start_or_an_unknown_anchor_is_refused() {
    let mut sequence = loaded("ab");
    let skipping = SeqOp::Insert {
        after: None,
        client: ALICE,
        start: 3,
        items: b"x".to_vec(),
    };
    let unknown = SeqOp::Insert {
        after: Some(Pos {
            client: BOB,
            offset: 0,
        }),
        client: ALICE,
        start: 0,
        items: b"x".to_vec(),
    };

    assert_eq!(sequence.apply(&skipping), None);
    assert_eq!(sequence.apply(&unknown), None);
    assert_eq!(sequence.inverse(&unknown), None);
    assert_eq!(text(&sequence), "ab");
}
