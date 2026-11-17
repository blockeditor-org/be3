use super::*;

#[test]
fn an_access_root_checks_only_as_its_creator_signed_it() {
    let creator = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let block = [7; 16];
    let root = Entry::root(&creator, [1; 16], LogKind::Block, block);
    assert_eq!(
        root.checked_root(LogKind::Block, block)
            .map(|root| root.identity),
        Ok(creator.identity()),
        "a root checks as its creator's"
    );
    assert_eq!(
        Entry::decode(&root.encode()).map(|entry| entry.hash()),
        Ok(root.hash()),
        "it round-trips, hash and all"
    );
    assert!(
        root.checked_root(LogKind::Block, [8; 16]).is_err(),
        "it is the root of its own log only"
    );
    assert!(
        root.checked_root(LogKind::Organization, block).is_err(),
        "a block's root never checks as an organization's"
    );

    let mut claimed = root.clone();
    let Record::Root(inner) = &mut claimed.record else {
        panic!("a root's record is a root");
    };
    inner.identity = other.identity();
    assert!(
        claimed.checked_root(LogKind::Block, block).is_err(),
        "naming other keys than it was signed with, it does not check"
    );
    assert_ne!(claimed.hash(), root.hash(), "and hashes differently");

    let forged = Entry::root(&other, [1; 16], LogKind::Block, block);
    assert!(
        forged.checked_root(LogKind::Block, block).is_ok(),
        "someone else's root is well formed"
    );
    assert_ne!(
        forged.hash(),
        root.hash(),
        "but its hash is not the one pinned for the block"
    );

    assert!(
        !creator
            .signing()
            .verifies_authored(block, &root.encode(), &root.signature),
        "a root's signature is not one over something authored"
    );

    let carried = CarriedKey {
        key: [3; 32],
        block,
        root: Some(root.hash()),
        heads: Heads {
            block: Some(root.hash()),
            organization: Some([9; 32]),
        },
    };
    assert_eq!(CarriedKey::decode(&carried.encode()), Ok(carried));
    let unvouched = CarriedKey {
        root: None,
        heads: Heads::default(),
        ..carried
    };
    assert_eq!(
        CarriedKey::decode(&unvouched.encode()),
        Ok(unvouched),
        "a key whose sender holds no root carries none"
    );
    let mut truncated = carried.encode();
    truncated.pop();
    assert!(CarriedKey::decode(&truncated).is_err());
    assert!(
        CarriedKey::decode(&[3; 32]).is_err(),
        "a bare key carries no block and no root"
    );
}
