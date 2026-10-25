use super::*;

#[test]
fn a_follower_resubmits_only_what_the_server_never_sequenced() {
    let mut server = Sequencer::new(None);
    let mut follower = Follower::new(7);

    let first = follower.submit(b"insert one".to_vec());
    let second = follower.submit(b"insert two".to_vec());
    assert_eq!(follower.pending(), 2);
    assert_eq!((first.base, second.base), (0, 0));

    let sequenced = server
        .submit(first.id, first.base, false, first.payload.clone())
        .unwrap();
    assert!(
        !follower.accepted(sequenced.id, sequenced.sequence),
        "an echo of our own operation was taken for a peer's"
    );
    assert_eq!(follower.pending(), 1);
    assert_eq!(follower.applied(), 1);

    let resubmitted = follower.resynchronize();
    assert_eq!(resubmitted.len(), 1);
    assert_eq!(resubmitted[0].id, second.id);
    assert_eq!(resubmitted[0].base, 1);
    let again = resubmitted[0].clone();
    assert!(
        server
            .submit(again.id, again.base, false, again.payload.clone())
            .is_some()
    );
    assert!(
        server
            .submit(again.id, again.base, false, again.payload)
            .is_none(),
        "a resubmission after a reconnect landed twice"
    );

    let peer = server
        .submit(id(9, 1), 2, false, b"elsewhere".to_vec())
        .unwrap();
    assert!(follower.accepted(peer.id, peer.sequence));

    follower.submit(b"three".to_vec());
    follower.sequenced_through(server.log(7).counter);
    assert_eq!(follower.pending(), 1, "an unsequenced edit was forgotten");
    follower.sequenced_through(3);
    assert_eq!(follower.pending(), 0);
}
