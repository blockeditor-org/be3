use super::*;

#[test]
fn a_seal_trims_the_log_but_keeps_what_later_operations_rebase_onto() {
    let head = write(&commits(), "base\n", 1_000, None);
    let mut server = Sequencer::new(Some(head));
    for counter in 1..=4 {
        server
            .submit(id(1, counter), counter - 1, false, vec![])
            .unwrap();
    }
    let late = server.submit(id(2, 1), 2, false, vec![]).unwrap();
    assert_eq!(late.sequence, 5);

    let sealed = write(&commits(), "base\nfour\n", 2_000, Some(head));
    assert!(server.sealed(SessionSeal {
        head: Some(sealed),
        sequence: 4,
        state: Some(b"state".to_vec()),
    }));
    let log = server.log(1);
    assert_eq!(log.seal.sequence, 4);
    assert_eq!(log.seal.state.as_deref(), Some(&b"state"[..]));
    assert_eq!(
        log.ops.iter().map(|op| op.sequence).collect::<Vec<_>>(),
        [3, 4, 5],
        "the operations the fifth rebases onto were dropped"
    );
    assert_eq!(server.start(), 2);

    let next = server.submit(id(1, 5), 5, false, vec![]).unwrap();
    assert_eq!(next.floor, 2);
    assert!(
        !server.sealed(SessionSeal {
            head: Some(head),
            sequence: 3,
            state: None,
        }),
        "an older seal replaced a newer one"
    );
    assert!(server.sealed(SessionSeal {
        head: Some(sealed),
        sequence: 6,
        state: None,
    }));
    assert!(server.is_clean());
    assert_eq!(server.held(), 0);
}
