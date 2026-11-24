use super::*;

/// Ownership moves in two records: the owner offers the block, which moves no one, and the
/// account it offered it to accepts, naming itself with the keys the offer named; only then does
/// the new owner manage the block, no grant name them, and the old owner keep Edit and grant no
/// more. The owner withdraws an offer by naming itself. A proof path that reduces the acceptance
/// still leaves the old owner no Manage (guides/blocks/keys.md, "Owner records").
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
    let owner = |by: &RecoverySecret, signer, previous: &Entry, account, identity, lowers| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous.hash()),
            lowers,
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
            owner(&heir, heir_id, &root, heir_id, heir.identity(), true)
        ])
        .is_err(),
        "someone takes a block no one offered them"
    );
    assert!(
        check(&[
            root.clone(),
            owner(&heir, heir_id, &root, other_id, other.identity(), false)
        ])
        .is_err(),
        "someone who does not own the block offers it"
    );
    assert!(
        check(&[
            root.clone(),
            owner(&creator, creator_id, &root, heir_id, heir.identity(), true)
        ])
        .is_err(),
        "an offer says it lowers someone"
    );

    // The offer moves no one: the creator still owns and manages the block.
    let offer = owner(&creator, creator_id, &root, heir_id, heir.identity(), false);
    let offered = check(&[root.clone(), offer.clone()]).unwrap();
    assert_eq!(offered.owner, Some(creator_id));
    assert_eq!(offered.offered, Some((heir_id, heir.identity())));
    assert_eq!(offered.manager(LogKind::Block, heir_id), None);
    assert!(!offered.outdates(&root.hash()));
    assert!(
        check(&[
            root.clone(),
            offer.clone(),
            owner(&other, other_id, &offer, other_id, other.identity(), true)
        ])
        .is_err(),
        "someone the block was not offered to takes it"
    );
    assert!(
        check(&[
            root.clone(),
            offer.clone(),
            owner(&heir, heir_id, &offer, heir_id, other.identity(), true)
        ])
        .is_err(),
        "the account offered the block takes it with other keys than the offer named"
    );
    let withdrawn = owner(
        &creator,
        creator_id,
        &offer,
        creator_id,
        creator.identity(),
        false,
    );
    let after = check(&[root.clone(), offer.clone(), withdrawn.clone()]).unwrap();
    assert_eq!(after.offered, None, "a withdrawn offer stands");
    assert!(
        check(&[
            root.clone(),
            offer.clone(),
            withdrawn.clone(),
            owner(&heir, heir_id, &withdrawn, heir_id, heir.identity(), true)
        ])
        .is_err(),
        "a withdrawn offer is accepted"
    );
    let unlowered = owner(&heir, heir_id, &offer, heir_id, heir.identity(), false);
    assert!(
        check(&[root.clone(), offer.clone(), unlowered]).is_err(),
        "an acceptance says it lowers no one, so a path reducing it hides that"
    );

    let accepted = owner(&heir, heir_id, &offer, heir_id, heir.identity(), true);
    let log = [root.clone(), offer.clone(), accepted.clone()];
    let checked = check(&log).unwrap();
    assert_eq!(checked.owner, Some(heir_id));
    assert_eq!(checked.offered, None);
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
        checked.outdates(&offer.hash()),
        "an accepted transfer outdates the epochs before it"
    );

    let late = grant(
        &creator,
        creator_id,
        &accepted,
        other_id,
        other.identity(),
        false,
    );
    assert!(
        check(&[root.clone(), offer.clone(), accepted.clone(), late.clone()]).is_err(),
        "the old owner grants after the transfer"
    );
    let onward = grant(&heir, heir_id, &accepted, other_id, other.identity(), false);
    assert!(
        check(&[root.clone(), offer.clone(), accepted.clone(), onward]).is_ok(),
        "the new owner grants"
    );
    let demoting = Entry::sign(
        &creator,
        creator_id,
        block,
        Some(accepted.hash()),
        true,
        Record::Grant(Grant {
            grantee: heir_id,
            identity: heir.identity(),
            level: Level::None,
            authority: Authority::Log,
        }),
    );
    assert!(
        check(&[root.clone(), offer.clone(), accepted.clone(), demoting]).is_err(),
        "a grant names the owner"
    );
    let back = owner(
        &creator,
        creator_id,
        &accepted,
        other_id,
        other.identity(),
        false,
    );
    assert!(
        check(&[root.clone(), offer.clone(), accepted.clone(), back]).is_err(),
        "the old owner offers the block again"
    );

    // A server that reduces the acceptance on a path still leaves the old owner no Manage, so
    // a grant they sign after it, which it shows whole, does not check.
    let path = [
        Step::Entry(root.clone()),
        Step::Entry(offer.clone()),
        Step::Reduced(accepted.reduced()),
        Step::Entry(late),
    ];
    assert!(
        check_log(LogKind::Block, block, root.hash(), &path, &none).is_err(),
        "a path hiding the transfer lets the old owner grant"
    );
    let path = [
        Step::Entry(root.clone()),
        Step::Entry(offer.clone()),
        Step::Reduced(accepted.reduced()),
    ];
    let hidden = check_log(LogKind::Block, block, root.hash(), &path, &none).unwrap();
    assert_eq!(hidden.owner, None, "a reduced transfer names an owner");
    assert_eq!(hidden.manager(LogKind::Block, creator_id), None);

    // An honest path proves the offer and its acceptance whole to any reader.
    let served = proof_path(LogKind::Block, &log, other_id, []);
    assert!(
        matches!(served[1], Step::Entry(_)) && matches!(served[2], Step::Entry(_)),
        "a path reduces an owner record"
    );
}
