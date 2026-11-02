use super::*;

#[test]
fn a_keyring_opens_by_the_id_sealed_bytes_carry() {
    let keyring = keys(1);
    keyring.insert(id(2), ContentKey::from_bytes([2; 32]));
    let first = keyring.seal(id(1), b"under the first key").unwrap();
    let second = keyring.seal(id(2), b"under the second key").unwrap();
    assert_eq!(&first[..4], b"be3K");
    assert_eq!(sealed_key(&first).unwrap(), id(1));
    assert_eq!(sealed_key(&second).unwrap(), id(2));
    assert_eq!(keyring.open(&first).unwrap(), b"under the first key");
    assert_eq!(keyring.open(&second).unwrap(), b"under the second key");
    assert_eq!(
        keyring.seal(id(1), b"under the first key").unwrap(),
        first,
        "one key sealed the same bytes differently"
    );

    assert!(matches!(
        keyring.seal(id(3), b"no such key"),
        Err(StoreError::MissingKey(missing)) if missing == id(3)
    ));
    let elsewhere = keys(3).seal(id(3), b"a key this ring lacks").unwrap();
    assert!(matches!(
        keyring.open(&elsewhere),
        Err(StoreError::MissingKey(missing)) if missing == id(3)
    ));
    assert!(matches!(
        keyring.open(&first[4..]),
        Err(StoreError::Corrupt)
    ));
    let mut untagged = first.clone();
    untagged[0] = b'x';
    assert!(matches!(keyring.open(&untagged), Err(StoreError::Corrupt)));
}
