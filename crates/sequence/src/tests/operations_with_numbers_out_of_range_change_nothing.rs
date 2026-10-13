use super::*;

#[test]
fn operations_with_numbers_out_of_range_change_nothing() {
    let mut sequence = loaded("abc");
    let wrapping = Span {
        client: LOADED,
        start: u64::MAX,
        len: 2,
    };
    let nowhere = Pos {
        client: LOADED,
        offset: u64::MAX,
    };
    let refused = [
        SeqOp::Delete {
            spans: vec![wrapping],
        },
        SeqOp::Undelete {
            spans: vec![wrapping],
            items: vec![b'x'; 2],
        },
        SeqOp::Replace {
            spans: vec![wrapping],
            client: ALICE,
            start: 0,
            items: b"x".to_vec(),
        },
        SeqOp::Swap {
            hide: vec![wrapping],
            show: Vec::new(),
            items: Vec::new(),
        },
        SeqOp::Insert {
            after: Some(nowhere),
            client: ALICE,
            start: 0,
            items: b"x".to_vec(),
        },
        SeqOp::Insert {
            after: None,
            client: ALICE,
            start: u64::MAX,
            items: b"x".to_vec(),
        },
        SeqOp::Move {
            first: nowhere,
            last: nowhere,
            after: None,
        },
    ];

    for op in &refused {
        assert_eq!(sequence.inverse(op), None, "{op:?}");
        assert_eq!(sequence.apply(op), None, "{op:?}");
    }
    assert_eq!(text(&sequence), "abc");
    check(&sequence);

    let mut wrapped = sequence.state();
    wrapped.fragments.push(super::super::Fragment {
        client: LOADED,
        start: u64::MAX,
        len: 2,
        visible: false,
    });
    assert!(Sequence::from_state(wrapped, &sequence.items()).is_err());
}
