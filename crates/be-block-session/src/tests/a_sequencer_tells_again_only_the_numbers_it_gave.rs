use super::*;

/// An operation submitted again is told the number this sequencer gave it, and one it inherited
/// but could not number (it waits for a reload) none: the old owner's number is not this one's.
#[test]
fn a_sequencer_tells_again_only_the_numbers_it_gave() {
    let mut owner = Sequencer::continuing(None, 5, 0);
    let taken = OpId {
        client: 7,
        counter: 1,
    };
    let op = owner
        .accept(taken, Payload::Inline(b"typed".to_vec()))
        .expect("a new id is taken");
    assert_eq!(owner.accepted_as(taken), Some(op.sequence));
    assert!(
        owner
            .accept(taken, Payload::Inline(b"typed".to_vec()))
            .is_none()
    );

    let skipped = SessionOp {
        id: OpId {
            client: 8,
            counter: 1,
        },
        sequence: op.sequence + 3,
        payload: Payload::Inline(b"accepted late".to_vec()),
    };
    assert_eq!(owner.inherit(&skipped), Inherited::Unnumbered);
    assert_eq!(owner.accepted_as(skipped.id), None);
    assert!(
        owner.accept(skipped.id, skipped.payload.clone()).is_none(),
        "it is still taken, only not numbered here"
    );
}
