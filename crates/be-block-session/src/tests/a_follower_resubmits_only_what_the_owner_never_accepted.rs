use super::*;

#[test]
fn a_follower_resubmits_only_what_the_owner_never_accepted() {
    let head = write(&commits(), "base\n", 1_000, None);
    let mut owner = Sequencer::new(Some(head));
    let mut follower = Follower::new(7);

    let SessionMessage::Submit {
        id: first, payload, ..
    } = follower.submit(b"insert one".to_vec())
    else {
        panic!("a submission is a submit message");
    };
    let SessionMessage::Submit { id: second, .. } = follower.submit(b"insert two".to_vec()) else {
        panic!("a submission is a submit message");
    };
    assert_eq!(follower.pending(), 2);

    let accepted = owner.accept(first, payload.clone()).unwrap();
    assert_eq!(accepted.sequence, 1);
    assert!(
        owner.accept(first, payload).is_none(),
        "an op was applied twice"
    );
    assert!(
        !follower.accepted(&accepted),
        "an echo of our own op was replayed"
    );
    assert_eq!(follower.pending(), 1);
    assert_eq!(follower.applied(), 1);

    let resubmitted = follower.resynchronize(owner.sequence());
    assert_eq!(resubmitted.len(), 1);
    let SessionMessage::Submit { id, payload, .. } = resubmitted.into_iter().next().unwrap() else {
        panic!("a resubmission is a submit message");
    };
    assert_eq!(id, second);
    assert!(owner.accept(id, payload).is_some());
    assert_eq!(owner.pending_operations(), 2);
    assert!(!owner.is_clean());

    let peer = owner
        .accept(
            OpId {
                client: 9,
                counter: 1,
            },
            b"from elsewhere".to_vec(),
        )
        .unwrap();
    assert!(follower.accepted(&peer), "a peer's op should be applied");
    assert_eq!(owner.since(1).len(), 2);

    let sealed = write(&commits(), "base\nsealed\n", 2_000, Some(head));
    owner.sealed(sealed);
    assert!(owner.is_clean());
    assert_eq!(owner.head(), Some(sealed));
}
