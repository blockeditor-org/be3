use super::*;

/// A manager's device that appends a lowering judges by it at once (guides/blocks/keys.md,
/// "Outdated epochs"), not by the log it read before signing it: the epoch it holds counts as
/// outdated, the log is due a read before the next seal, and a log served without the lowering
/// is withheld, until one that holds it is checked.
#[test]
fn a_lowering_this_device_appended_outdates_its_epochs_before_any_log_shows_it() {
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
    let check = |steps: &[Step]| {
        be_block_keys::check_log(
            LogKind::Block,
            block.into_bytes(),
            root.hash(),
            steps,
            &|_| None,
        )
        .unwrap()
    };
    let grant = |steps: &[Step], level: Level| -> Entry {
        check(steps)
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

    let book = crate::KeyBook::new(creator_id, Uuid::new_v4(), creator.clone(), Keyring::new());
    book.pin_root(block, root.hash());
    book.note_log(block, &check(&log), &log);
    let (scope, _) = book.mint_scope(block, None).unwrap();
    assert!(!book.outdated(scope, block));

    let raised = grant(&log, Level::Edit);
    assert_eq!(
        book.appended(&raised.encode()),
        None,
        "a grant that lowers no one changes nothing"
    );
    assert!(!book.outdated(scope, block));

    let revoked = grant(&log, Level::None);
    assert_eq!(book.appended(&revoked.encode()), Some(block));
    assert!(
        book.outdated(scope, block),
        "the epoch the revoked reader holds is written under still"
    );
    assert!(
        book.shown_outdated(scope, block),
        "an edit waiting to be sent goes up under the epoch the revoked reader holds"
    );
    assert_eq!(book.logs_to_read(scope, block), vec![block]);
    assert!(
        matches!(
            book.served(block, &check(&log)),
            crate::keys::Served::Withheld
        ),
        "a log served without the revocation is withheld"
    );

    let mut after = log.clone();
    after.push(revoked.into());
    assert!(matches!(
        book.served(block, &check(&after)),
        crate::keys::Served::Take
    ));
    book.note_log(block, &check(&after), &after);
    assert!(
        book.logs_to_read(scope, block).is_empty(),
        "a log holding the revocation is read again"
    );
    assert!(
        book.outdated(scope, block),
        "the log outdates the epoch as the revocation did"
    );
}
