use super::*;

#[test]
fn tampered_bytes_fail_to_open() {
    let vault = vault(6);
    let sealed = vault.seal(b"the quick brown fox");
    assert_eq!(vault.open(&sealed).unwrap(), b"the quick brown fox");

    let mut flipped = sealed.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 0x01;
    assert!(matches!(vault.open(&flipped), Err(StoreError::Corrupt)));

    assert!(matches!(vault.open(&sealed[..4]), Err(StoreError::Corrupt)));

    let missing = Manifest {
        content_type: CONTENT,
        length: 4,
        chunks: vec![ChunkRef {
            hash: Hash::of(b"absent"),
            length: 4,
        }],
    };
    assert!(matches!(
        vault.read(&missing),
        Err(StoreError::MissingObject(_))
    ));
}
