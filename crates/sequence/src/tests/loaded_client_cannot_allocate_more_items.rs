use super::*;

#[test]
fn loaded_client_cannot_allocate_more_items() {
    for original in ["", "ab"] {
        let mut sequence = loaded(original);
        let before = order(&sequence);
        assert_eq!(sequence.insert(LOADED, 0, b"x".to_vec()), None);
        assert_eq!(
            sequence.replace(LOADED, 0..original.len(), b"x".to_vec()),
            None
        );
        for operation in [
            SeqOp::Insert {
                after: None,
                client: LOADED,
                start: original.len() as u64,
                items: b"x".to_vec(),
            },
            SeqOp::Replace {
                spans: sequence.spans(0..original.len()),
                client: LOADED,
                start: original.len() as u64,
                items: b"x".to_vec(),
            },
        ] {
            assert!(!sequence.applies(&operation));
            assert_eq!(sequence.inverse(&operation), None);
            assert_eq!(sequence.apply(&operation), None);
            assert_eq!(order(&sequence), before);
            assert_eq!(sequence.next_offset(LOADED), original.len() as u64);
            assert_eq!(text(&sequence), original);
        }
        typed(&mut sequence, ALICE, 0, "x");
        assert_eq!(text(&sequence), format!("x{original}"));
        assert_eq!(sequence.next_offset(LOADED), original.len() as u64);
    }
}
