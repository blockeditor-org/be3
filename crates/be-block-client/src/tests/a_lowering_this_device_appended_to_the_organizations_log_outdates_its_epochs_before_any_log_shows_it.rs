use super::*;

use be_block_keys::Administrator;

/// As a_lowering_this_device_appended_outdates_its_epochs_before_any_log_shows_it, for the
/// organization's log: an administrator's device that unmakes another administrator judges by
/// the demotion at once. Every epoch escrow may hold is outdated and demoted, the organization's
/// log is due a read before the next seal, and one served without the demotion is withheld,
/// until one that holds it is checked. An owner-only scope, which no escrow holds, stays current.
#[test]
fn a_lowering_this_device_appended_to_the_organizations_log_outdates_its_epochs_before_any_log_shows_it()
 {
    let creator = RecoveryPhrase::generate().secret();
    let deputy = RecoveryPhrase::generate().secret();
    let (creator_id, deputy_id) = (Uuid::new_v4(), Uuid::new_v4());
    let (organization, block, own) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    let organization_root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Organization,
        organization.into_bytes(),
    );
    let check_organization = |steps: &[Step]| {
        be_block_keys::check_log(
            LogKind::Organization,
            organization.into_bytes(),
            organization_root.hash(),
            steps,
            &|_| None,
        )
        .unwrap()
    };
    let administrator = |steps: &[Step], administrator: bool| -> Entry {
        check_organization(steps)
            .next(
                &creator,
                creator_id.into_bytes(),
                organization.into_bytes(),
                Record::Administrator(Administrator {
                    account: deputy_id.into_bytes(),
                    identity: deputy.identity(),
                    administrator,
                }),
            )
            .unwrap()
    };
    let mut log: Vec<Step> = vec![organization_root.clone().into()];
    log.push(administrator(&log, true).into());

    let block_root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Block,
        block.into_bytes(),
    );
    let block_log: Vec<Step> = vec![block_root.clone().into()];
    let block_checked = be_block_keys::check_log(
        LogKind::Block,
        block.into_bytes(),
        block_root.hash(),
        &block_log,
        &|_| None,
    )
    .unwrap();

    let book = crate::KeyBook::new(creator_id, organization, creator.clone(), Keyring::new());
    book.set_pinned(Some(crate::contacts::Pinned {
        root: Some(organization_root.hash()),
        escrow: None,
        made: 0,
    }));
    book.note_log(organization, &check_organization(&log), &log);
    book.set_organization(Some(&check_organization(&log)));
    book.pin_root(block, block_root.hash());
    book.note_log(block, &block_checked, &block_log);
    let (scope, _) = book.mint_scope(block, None).unwrap();
    let (own_scope, _) = book.mint_own_scope(own).unwrap();
    assert!(!book.outdated(scope, block) && !book.demoted(scope, block));
    assert!(book.logs_to_read(scope, block).is_empty());

    let demotion = administrator(&log, false);
    assert!(demotion.lowers);
    assert_eq!(book.appended(&demotion.encode()), Some(organization));
    assert!(
        book.outdated(scope, block),
        "an epoch the demoted administrator may hold through escrow is outdated"
    );
    assert!(
        book.demoted(scope, block),
        "key work rotates an epoch the demotion outdates at once"
    );
    assert_eq!(
        book.logs_to_read(scope, block),
        vec![organization],
        "the organization's log is read again before the next seal"
    );
    assert!(
        matches!(
            book.served(organization, &check_organization(&log)),
            crate::keys::Served::Withheld
        ),
        "a log served without the demotion is withheld"
    );
    assert!(
        !book.outdated(own_scope, own) && !book.demoted(own_scope, own),
        "an owner-only scope, which no escrow holds, stays current"
    );

    let mut after = log.clone();
    after.push(demotion.into());
    assert!(matches!(
        book.served(organization, &check_organization(&after)),
        crate::keys::Served::Take
    ));
    book.note_log(organization, &check_organization(&after), &after);
    assert!(
        book.logs_to_read(scope, block).is_empty(),
        "a log holding the demotion is not read again"
    );
    assert!(
        book.outdated(scope, block),
        "the log outdates the epoch as the demotion did"
    );
}
