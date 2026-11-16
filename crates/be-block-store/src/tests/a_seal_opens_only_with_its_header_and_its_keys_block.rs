use super::*;

/// A seal authenticates its header and the block its key is of: the same key bytes held as a
/// key of another block open nothing, and neither does a header relabelled to another key id,
/// even one holding the same key bytes.
#[test]
fn a_seal_opens_only_with_its_header_and_its_keys_block() {
    let block = Uuid::from_u128(0xb10c);
    let sealer = Keyring::new();
    sealer.insert(id(1), ContentKey::from_bytes([1; 32]), block);
    let sealed = sealer.seal(id(1), b"a key's own block").unwrap();
    assert_eq!(sealer.open(&sealed).unwrap(), b"a key's own block");

    let elsewhere = Keyring::new();
    elsewhere.insert(id(1), ContentKey::from_bytes([1; 32]), Uuid::from_u128(0xe15e));
    assert!(
        matches!(elsewhere.open(&sealed), Err(StoreError::Corrupt)),
        "the key held as another block's opened it"
    );

    let relabelled_ring = Keyring::new();
    relabelled_ring.insert(id(2), ContentKey::from_bytes([1; 32]), block);
    let mut relabelled = sealed.clone();
    relabelled[4..20].copy_from_slice(id(2).as_bytes());
    assert!(
        matches!(relabelled_ring.open(&relabelled), Err(StoreError::Corrupt)),
        "a header relabelled to another key id opened"
    );
}
