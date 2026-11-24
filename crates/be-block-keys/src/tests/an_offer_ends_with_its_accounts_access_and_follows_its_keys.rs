use super::*;

/// An offer of ownership is accepted only with the account's current keys: once it takes a new
/// phrase the offer follows it to the new keys and the old ones accept nothing. Revoking the
/// account, or banning it, ends the offer (guides/blocks/keys.md, "Owner records").
#[test]
fn an_offer_ends_with_its_accounts_access_and_follows_its_keys() {
    let creator = RecoveryPhrase::generate().secret();
    let heir = RecoveryPhrase::generate().secret();
    let renewed = RecoveryPhrase::generate().secret();
    let (creator_id, heir_id) = ([1; 16], [2; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let check =
        |entries: &[Entry]| check_log(LogKind::Block, block, entries[0].hash(), entries, &none);
    let sign = |by: &RecoverySecret, signer, previous: &Entry, lowers, record| {
        Entry::sign(by, signer, block, Some(previous.hash()), lowers, record)
    };
    let grant = |level, identity| {
        Record::Grant(Grant {
            grantee: heir_id,
            identity,
            level,
            authority: Authority::Log,
        })
    };
    let owner = |identity| {
        Record::Owner(Owner {
            account: heir_id,
            identity,
        })
    };

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let granted = sign(&creator, creator_id, &root, false, grant(Level::View, heir.identity()));
    let offer = sign(&creator, creator_id, &granted, false, owner(heir.identity()));
    let log = vec![root.clone(), granted.clone(), offer.clone()];
    assert_eq!(
        check(&log).unwrap().offered,
        Some((heir_id, heir.identity()))
    );

    // The heir takes a new phrase: the offer moves to its new keys.
    let retired = Entry::retirement(&heir, &renewed, heir_id, block, offer.hash());
    let mut after = log.clone();
    after.push(retired.clone());
    assert_eq!(
        check(&after).unwrap().offered,
        Some((heir_id, renewed.identity())),
        "the offer follows the account's keys"
    );
    let stale = sign(&heir, heir_id, &retired, true, owner(heir.identity()));
    assert!(
        check(&[after.clone(), vec![stale]].concat()).is_err(),
        "the retired keys accept the offer"
    );
    let accepted = sign(&renewed, heir_id, &retired, true, owner(renewed.identity()));
    assert_eq!(
        check(&[after, vec![accepted]].concat()).unwrap().owner,
        Some(heir_id),
        "the account's new keys accept it"
    );

    // Revoked, or banned, the heir holds no offer.
    let revoked = sign(&creator, creator_id, &offer, true, grant(Level::None, heir.identity()));
    let ban = sign(
        &creator,
        creator_id,
        &offer,
        false,
        Record::Ban(Ban {
            account: heir_id,
            authority: Authority::Log,
        }),
    );
    for (what, ending) in [("revoked", revoked), ("banned", ban)] {
        let mut ended = log.clone();
        ended.push(ending.clone());
        assert_eq!(check(&ended).unwrap().offered, None, "{what}, the offer stands");
        let late = sign(&heir, heir_id, &ending, true, owner(heir.identity()));
        assert!(
            check(&[ended, vec![late]].concat()).is_err(),
            "{what}, the heir accepts the block"
        );
    }
}
