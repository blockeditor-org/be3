use super::*;

use be_block_keys::{CarriedKey, Entry, Heads, LogKind, RecoveryPhrase};
use be_block_protocol::{KeyKind, KeyRow, KeySet, WrappedBy, WrappedKey};
use be_block_store::{ContentKey, Keyring};

/// A key is judged by every key it was opened through, up to the most any honest tree holds
/// (`keys::MAX_PATH`): a chain longer than that, each key naming the log's current head, is
/// judged outdated, as is a key opened through another in a scope whose log this device
/// never read nor kept.
#[test]
fn a_key_opened_through_a_chain_longer_than_any_tree_is_outdated() {
    let identity = RecoveryPhrase::generate().secret();
    let account = Uuid::new_v4();
    let block = Uuid::new_v4();
    let book = crate::KeyBook::new(account, Uuid::new_v4(), identity.clone(), Keyring::new());
    let root = Entry::root(
        &identity,
        account.into_bytes(),
        LogKind::Block,
        block.into_bytes(),
    );
    book.pin_root(block, root.hash());
    let checked = be_block_keys::check_log(
        LogKind::Block,
        block.into_bytes(),
        root.hash(),
        &[root.clone()],
        &|_| None,
    )
    .unwrap();
    book.note_log(block, &checked, &[root.clone().into()]);
    let (scope, _) = book.mint_scope(block, None).unwrap();

    // Each key wrapped under the one before, the first under the scope key.
    let sealer = Keyring::new();
    sealer.insert(
        scope,
        ContentKey::from_bytes(*book.ring().get(scope).unwrap().as_bytes()),
        block,
    );
    let mut set = KeySet::default();
    let mut chain = vec![scope];
    for at in 0..crate::keys::MAX_PATH + 4 {
        let key = Uuid::new_v4();
        let carried = CarriedKey {
            key: [at as u8; 32],
            label: crate::tests::planted(key, "data", at as u64 + 2),
            block: block.into_bytes(),
            root: Some(root.hash()),
            heads: Heads {
                block: Some(root.hash()),
                organization: None,
            },
        };
        let under = *chain.last().unwrap();
        set.keys.push(KeyRow {
            id: key,
            block,
            kind: KeyKind::Data,
            epoch: at as u64 + 2,
        });
        set.wrapped.push(WrappedKey {
            key,
            wrapped_by: WrappedBy::Key(under),
            sender: account,
            sealed: sealer.seal(under, &carried.encode()).unwrap(),
        });
        sealer.insert(key, ContentKey::from_bytes([at as u8; 32]), block);
        chain.push(key);
    }
    book.absorb(set);
    let short = chain[4];
    let long = *chain.last().unwrap();
    assert!(book.trusted(long));
    assert!(
        !book.outdated(short, block),
        "a short chain under a current epoch is outdated"
    );
    assert!(
        book.outdated(long, block),
        "a chain longer than any tree is judged current"
    );

    // A scope whose log this device neither read nor kept: a key opened through another there
    // cannot be judged, so it is outdated; one made here is not.
    let unread = Uuid::new_v4();
    book.pin_root(unread, root.hash());
    let (unread_scope, _) = book.mint_scope(unread, None).unwrap();
    let opened = Uuid::new_v4();
    sealer.insert(
        unread_scope,
        ContentKey::from_bytes(*book.ring().get(unread_scope).unwrap().as_bytes()),
        unread,
    );
    let carried = CarriedKey {
        key: [1; 32],
        label: crate::tests::planted(opened, "data", 2),
        block: unread.into_bytes(),
        root: None,
        heads: Heads::default(),
    };
    book.absorb(KeySet {
        keys: vec![KeyRow {
            id: opened,
            block: unread,
            kind: KeyKind::Data,
            epoch: 2,
        }],
        wrapped: vec![WrappedKey {
            key: opened,
            wrapped_by: WrappedBy::Key(unread_scope),
            sender: account,
            sealed: sealer.seal(unread_scope, &carried.encode()).unwrap(),
        }],
    });
    assert!(book.trusted(opened));
    assert!(!book.outdated(unread_scope, unread));
    assert!(book.outdated(opened, unread));
}
