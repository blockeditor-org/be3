use super::*;

/// Ownership moves only by the owner's record; the new owner manages the block, no grant names
/// them, and the old owner keeps Edit and grants no more. A proof path that reduces the transfer
/// still leaves the old owner no Manage (plans/signed_access.md, step 5).
#[test]
fn an_owner_record_checks_only_as_the_owner_signed_it_and_a_path_cannot_hide_one() {
    let creator = RecoveryPhrase::generate().secret();
    let heir = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let (creator_id, heir_id, other_id) = ([1; 16], [2; 16], [3; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let check =
        |entries: &[Entry]| check_log(LogKind::Block, block, entries[0].hash(), entries, &none);

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let owner = |by: &RecoverySecret, signer, previous: &Entry, account, identity| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous.hash()),
            true,
            Record::Owner(Owner { account, identity }),
        )
    };
    let grant = |by: &RecoverySecret, signer, previous: &Entry, grantee, identity, lowers| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous.hash()),
            lowers,
            Record::Grant(Grant {
                grantee,
                identity,
                level: Level::View,
                authority: Authority::Log,
            }),
        )
    };

    assert!(
        check(&[
            root.clone(),
            owner(&heir, heir_id, &root, heir_id, heir.identity())
        ])
        .is_err(),
        "someone who does not own the block takes it"
    );
    assert!(
        check(&[
            root.clone(),
            owner(&creator, creator_id, &root, creator_id, creator.identity())
        ])
        .is_err(),
        "an owner gives the block to themselves"
    );
    let unlowered = Entry::sign(
        &creator,
        creator_id,
        block,
        Some(root.hash()),
        false,
        Record::Owner(Owner {
            account: heir_id,
            identity: heir.identity(),
        }),
    );
    assert!(
        check(&[root.clone(), unlowered]).is_err(),
        "a transfer says it lowers no one, so a path reducing it hides that"
    );

    let transfer = owner(&creator, creator_id, &root, heir_id, heir.identity());
    let log = [root.clone(), transfer.clone()];
    let checked = check(&log).unwrap();
    assert_eq!(checked.owner, Some(heir_id));
    assert_eq!(
        checked.manager(LogKind::Block, heir_id),
        Some(heir.signing())
    );
    assert_eq!(
        checked.grants.get(&creator_id),
        Some(&(Level::Edit, creator.identity())),
        "the old owner keeps Edit"
    );
    assert!(
        checked.outdates(&root.hash()),
        "a transfer outdates the epochs before it"
    );

    let late = grant(
        &creator,
        creator_id,
        &transfer,
        other_id,
        other.identity(),
        false,
    );
    assert!(
        check(&[root.clone(), transfer.clone(), late.clone()]).is_err(),
        "the old owner grants after the transfer"
    );
    let onward = grant(&heir, heir_id, &transfer, other_id, other.identity(), false);
    assert!(
        check(&[root.clone(), transfer.clone(), onward]).is_ok(),
        "the new owner grants"
    );
    let demoting = Entry::sign(
        &creator,
        creator_id,
        block,
        Some(transfer.hash()),
        true,
        Record::Grant(Grant {
            grantee: heir_id,
            identity: heir.identity(),
            level: Level::None,
            authority: Authority::Log,
        }),
    );
    assert!(
        check(&[root.clone(), transfer.clone(), demoting]).is_err(),
        "a grant names the owner"
    );
    let back = owner(&creator, creator_id, &transfer, other_id, other.identity());
    assert!(
        check(&[root.clone(), transfer.clone(), back]).is_err(),
        "the old owner gives the block away again"
    );

    // A server that reduces the transfer on a path still leaves the old owner no Manage, so a
    // grant they sign after it, which it shows whole, does not check.
    let path = [
        Step::Entry(root.clone()),
        Step::Reduced(transfer.reduced()),
        Step::Entry(late),
    ];
    assert!(
        check_log(LogKind::Block, block, root.hash(), &path, &none).is_err(),
        "a path hiding the transfer lets the old owner grant"
    );
    let path = [Step::Entry(root.clone()), Step::Reduced(transfer.reduced())];
    let hidden = check_log(LogKind::Block, block, root.hash(), &path, &none).unwrap();
    assert_eq!(hidden.owner, None, "a reduced transfer names an owner");
    assert_eq!(hidden.manager(LogKind::Block, creator_id), None);

    // An honest path proves the transfer whole to any reader.
    let served = proof_path(LogKind::Block, &log, other_id, []);
    assert!(
        matches!(served[1], Step::Entry(_)),
        "a path reduces an owner record"
    );
}
