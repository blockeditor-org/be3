use super::*;

/// A log marked unsure (a lowering sent whose answer never came) is let go only by a read sent
/// after the newest mark, with no lowering to it in flight: a read captured before a second
/// lowering was sent may have been answered before that one landed.
#[test]
fn a_read_sent_before_a_newer_lowering_leaves_its_log_unsure() {
    let creator = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let (creator_id, reader_id) = (Uuid::new_v4(), Uuid::new_v4());
    let block = Uuid::new_v4();
    let root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Block,
        block.into_bytes(),
    );
    let grant = |steps: &[Step], level: Level| -> Entry {
        be_block_keys::check_log(
            LogKind::Block,
            block.into_bytes(),
            root.hash(),
            steps,
            &|_| None,
        )
        .unwrap()
        .next(
            &creator,
            creator_id.into_bytes(),
            block.into_bytes(),
            Record::Grant(Grant {
                grantee: reader_id.into_bytes(),
                identity: reader.identity(),
                level,
                authority: Authority::Log,
            }),
        )
        .unwrap()
    };
    let mut log: Vec<Step> = vec![root.clone().into()];
    log.push(grant(&log, Level::View).into());
    let revoked = grant(&log, Level::None).encode();
    let book = crate::KeyBook::new(creator_id, Uuid::new_v4(), creator.clone(), Keyring::new());

    // Kept from an earlier run, then marked again by a lowering sent since: a read captured
    // under the first mark clears nothing.
    book.set_unsure([block]);
    let first = book.is_unsure(block).unwrap();
    book.set_unsure([block]);
    assert!(!book.answered(block, first));
    assert!(
        book.awaiting(block),
        "a read from before the newer mark let the log go"
    );
    assert!(book.answered(block, book.is_unsure(block).unwrap()));
    assert!(!book.awaiting(block));

    // A lowering in flight: no read lets the log go until its write ends, and one that ends
    // unanswered leaves it for a read.
    let sending = book.sending(&revoked).unwrap();
    let captured = book.is_unsure(block).unwrap();
    assert!(
        !book.answered(block, captured),
        "a read let go a log with a lowering in flight"
    );
    let again = book.sending(&revoked).unwrap();
    assert!(
        !book.sent(sending, true),
        "an older write let go a newer mark"
    );
    assert!(!book.sent(again, false));
    assert!(book.awaiting(block));
    assert!(!book.answered(block, captured));
    assert!(book.answered(block, book.is_unsure(block).unwrap()));

    // One whose write the server answered lets it go at once.
    let sending = book.sending(&revoked).unwrap();
    assert!(book.awaiting(block));
    assert!(book.sent(sending, true));
    assert!(!book.awaiting(block));
}
