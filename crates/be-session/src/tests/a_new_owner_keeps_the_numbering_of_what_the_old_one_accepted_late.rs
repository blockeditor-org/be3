use super::*;

#[test]
fn a_new_owner_keeps_the_numbering_of_what_the_old_one_accepted_late() {
    let mut owner = Sequencer::continuing(None, 5, 0);
    let late = SessionOp {
        id: OpId {
            client: 1,
            counter: 6,
        },
        sequence: 6,
        payload: b"accepted by the old owner".to_vec(),
    };

    assert_eq!(owner.inherit(&late), Inherited::Continued);
    assert_eq!(owner.sequence(), 6);
    assert_eq!(owner.since(5), vec![late.clone()]);
    assert_eq!(owner.inherit(&late), Inherited::Duplicate);

    let mine = owner
        .accept(
            OpId {
                client: 2,
                counter: 1,
            },
            b"accepted by the new owner".to_vec(),
        )
        .unwrap();
    assert_eq!(mine.sequence, 7);

    let clash = SessionOp {
        id: OpId {
            client: 1,
            counter: 7,
        },
        sequence: 7,
        payload: b"numbered like the new owner's".to_vec(),
    };
    assert_eq!(owner.inherit(&clash), Inherited::Unnumbered);
    assert_eq!(owner.sequence(), 7, "a clashing number was taken");

    let resubmitted = SessionOp {
        id: mine.id,
        sequence: 8,
        payload: mine.payload.clone(),
    };
    assert_eq!(owner.inherit(&resubmitted), Inherited::Renumbered);

    let follower = Follower::continuing(1, 7);
    assert_eq!(follower.applied(), 7);
    assert_eq!(follower.catchup(), SessionMessage::Catchup { since: 7 });
}
