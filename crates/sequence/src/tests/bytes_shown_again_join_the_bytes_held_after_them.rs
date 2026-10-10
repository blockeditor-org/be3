use super::*;

#[test]
fn bytes_shown_again_join_the_bytes_held_after_them() {
    let mut sequence = loaded("");
    typed(&mut sequence, ALICE, 0, "abcdef");
    let delete = sequence.delete(1..4).expect("there is something to delete");
    applied(&mut sequence, &delete);
    let mut sequence = Sequence::from_state(sequence.state(), &sequence.items())
        .expect("a sequence's own state is well formed");
    let delete = sequence.delete(1..2).expect("there is something to delete");
    applied(&mut sequence, &delete);
    assert_eq!(text(&sequence), "af");

    let undelete = SeqOp::Undelete {
        spans: vec![Span {
            client: ALICE,
            start: 1,
            len: 4,
        }],
        items: b"bcde".to_vec(),
    };
    applied(&mut sequence, &undelete);

    assert_eq!(text(&sequence), "abcdef");
    let runs: Vec<(Pos, &[u8])> = sequence.runs().map(|run| (run.first, run.items)).collect();
    assert_eq!(
        runs,
        vec![(
            Pos {
                client: ALICE,
                offset: 0
            },
            &b"abcdef"[..]
        )]
    );
}
