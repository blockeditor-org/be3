use super::*;

#[test]
fn the_sequencer_numbers_each_submission_once() {
    let head = write(&commits(), "base\n", 1_000, None);
    let mut server = Sequencer::new(Some(head));
    assert_eq!(server.seal().head, Some(head));

    let first = server.submit(id(7, 1), 0, false, b"one".to_vec()).unwrap();
    assert_eq!((first.sequence, first.base, first.floor), (1, 0, 0));
    assert!(
        server.submit(id(7, 1), 0, false, b"one".to_vec()).is_none(),
        "a resubmitted operation was numbered twice"
    );
    let peer = server.submit(id(9, 1), 0, false, b"peer".to_vec()).unwrap();
    assert_eq!(peer.sequence, 2, "origins count separately");
    assert!(
        server
            .submit(id(7, 2), 5, false, b"ahead".to_vec())
            .is_none(),
        "an operation based on a sequence that does not exist yet was numbered"
    );
    assert_eq!(
        server
            .submit(id(7, 2), 2, false, b"two".to_vec())
            .unwrap()
            .sequence,
        3
    );
    assert_eq!(server.sequence(), 3);
    assert!(!server.is_clean());

    let log = server.log(7);
    assert_eq!(log.counter, 2);
    assert_eq!(
        log.ops.iter().map(|op| op.sequence).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(server.log(42).counter, 0);
}
