use super::*;

#[test]
fn session_state_round_trips_positions_and_tombstones() {
    let mut sequence = loaded("abcdef");
    typed(&mut sequence, ALICE, 3, "XY");
    let delete = sequence.delete(1..4).expect("there is something to delete");
    applied(&mut sequence, &delete);

    let mut adopted = Sequence::from_state(sequence.state()).expect("the state is well formed");
    assert_eq!(order(&adopted), order(&sequence));
    check(&adopted);

    let after_tombstone = SeqOp::Insert {
        after: Some(Pos {
            client: ALICE,
            offset: 0,
        }),
        client: BOB,
        start: 0,
        items: b"Z".to_vec(),
    };
    applied(&mut sequence, &after_tombstone);
    applied(&mut adopted, &after_tombstone);
    assert_eq!(order(&adopted), order(&sequence));
    assert_eq!(text(&adopted), "aZYdef");

    let mut beyond = sequence.state();
    beyond.buffers.insert(ALICE, b"X".to_vec());
    assert!(Sequence::from_state(beyond).is_err());
}
