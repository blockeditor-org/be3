use super::*;

#[test]
fn a_merged_delete_still_lands_beside_positions_a_refused_insert_left_unknown() {
    let mut authority = loaded("ab");
    let typed = authority.insert(BOB, 1, b"pq".to_vec()).unwrap();
    authority.apply(&typed);
    let refused = SeqOp::Insert {
        after: Some(Pos {
            client: 9,
            offset: 0,
        }),
        client: BOB,
        start: 2,
        items: b"rstuv".to_vec(),
    };
    assert!(authority.apply(&refused).is_none());

    let mut separate = authority.clone();
    let mut merged = SeqOp::Delete {
        spans: vec![Span {
            client: BOB,
            start: 2,
            len: 5,
        }],
    };
    let backspace = SeqOp::<u8>::Delete {
        spans: vec![Span {
            client: BOB,
            start: 1,
            len: 1,
        }],
    };
    separate.apply(&merged);
    separate.apply(&backspace);
    assert!(merged.absorb(backspace).is_none());
    authority.apply(&merged);

    assert_eq!(authority.items(), b"apb");
    assert_eq!(order(&authority), order(&separate));
}
