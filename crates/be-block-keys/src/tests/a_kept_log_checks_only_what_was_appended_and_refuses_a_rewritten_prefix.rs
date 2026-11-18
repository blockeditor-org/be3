use super::*;

/// The entries a long block log grows by: grants of View to accounts never named before, each
/// after the one before, from `previous`.
fn grants(
    creator: &RecoverySecret,
    creator_id: [u8; 16],
    block: [u8; 16],
    identity: PublicIdentity,
    mut previous: EntryHash,
    from: u64,
    count: u64,
) -> Vec<Entry> {
    (from..from + count)
        .map(|index| {
            let mut grantee = [0; 16];
            grantee[..8].copy_from_slice(&index.to_be_bytes());
            grantee[15] = 0xee;
            let entry = Entry::sign(
                creator,
                creator_id,
                block,
                Some(previous),
                false,
                Record::Grant(Grant {
                    grantee,
                    identity,
                    level: Level::View,
                    authority: Authority::Log,
                }),
            );
            previous = entry.hash();
            entry
        })
        .collect()
}

/// A device checks a log once and keeps what it checked: a second check of the log, grown by
/// one entry, verifies that entry's signature alone, and a log whose checked prefix the server
/// rewrote is refused, however its new entries check.
#[test]
fn a_kept_log_checks_only_what_was_appended_and_refuses_a_rewritten_prefix() {
    const LENGTH: u64 = 2_000;
    let creator = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret().identity();
    let (creator_id, block) = ([1; 16], [7; 16]);
    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let mut entries = vec![root.clone()];
    entries.extend(grants(
        &creator,
        creator_id,
        block,
        reader,
        root.hash(),
        1,
        LENGTH - 1,
    ));

    let mut kept = None;
    let first = verified(|| {
        kept = Some(check_log(LogKind::Block, block, root.hash(), &entries, &|_| None).unwrap());
    });
    let kept = kept.unwrap();
    assert!(first >= LENGTH as usize, "the first check verifies every entry");
    assert_eq!(kept.head, entries.last().unwrap().hash());

    // The device keeps it, as the refs database does, and reads it back.
    let kept: Checked = postcard::from_bytes(&postcard::to_stdvec(&kept).unwrap()).unwrap();
    let appended = grants(
        &creator,
        creator_id,
        block,
        reader,
        kept.head,
        LENGTH,
        1,
    );
    let grown: Vec<Entry> = entries.iter().chain(&appended).cloned().collect();
    let mut resumed = None;
    let second = verified(|| {
        resumed = Some(
            kept.clone()
                .resume(LogKind::Block, block, &grown, &|_| None)
                .unwrap(),
        );
    });
    assert_eq!(second, 1, "only the appended entry's signature is verified");
    let resumed = resumed.unwrap();
    assert_eq!(resumed.head, appended[0].hash());
    assert_eq!(
        resumed.level_as_of({
            let Record::Grant(grant) = &appended[0].record else { unreachable!() };
            grant.grantee
        }, &resumed.head),
        Some(Level::View)
    );

    // What is kept and extended is what a check from the root finds.
    let short = check_log(LogKind::Block, block, root.hash(), &entries[..1000], &|_| None).unwrap();
    assert_eq!(
        short
            .resume(LogKind::Block, block, &entries[..1001], &|_| None)
            .unwrap(),
        check_log(LogKind::Block, block, root.hash(), &entries[..1001], &|_| None).unwrap()
    );

    // The server rewrites an entry the device checked: the last it kept, so every entry still
    // names the one before, but the prefix does not end at the head that was kept.
    let last = LENGTH as usize - 1;
    let mut rewritten = grown.clone();
    rewritten[last] = grants(
        &creator,
        creator_id,
        block,
        reader,
        entries[last - 1].hash(),
        LENGTH * 2,
        1,
    )
    .remove(0);
    rewritten[last + 1] = grants(
        &creator,
        creator_id,
        block,
        reader,
        rewritten[last].hash(),
        LENGTH,
        1,
    )
    .remove(0);
    assert!(
        kept.clone()
            .resume(LogKind::Block, block, &rewritten, &|_| None)
            .is_err(),
        "a rewritten prefix is refused as forged"
    );
    // One in the middle: the entry after it no longer names it.
    let mut rewritten = grown.clone();
    rewritten[5] = grants(&creator, creator_id, block, reader, entries[4].hash(), LENGTH * 3, 1)
        .remove(0);
    assert!(
        kept.clone()
            .resume(LogKind::Block, block, &rewritten, &|_| None)
            .is_err()
    );
    assert!(
        kept.resume(LogKind::Block, block, &entries[..10], &|_| None)
            .is_err(),
        "a log cut short of what was kept is refused"
    );
}
