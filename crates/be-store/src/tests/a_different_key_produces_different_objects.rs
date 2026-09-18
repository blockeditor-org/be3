use super::*;

#[test]
fn a_different_key_produces_different_objects() {
    let data = pseudorandom(4096, 23);
    let first = vault(4);
    let second = vault(5);
    let one = first.write(CONTENT, &data).unwrap();
    let two = second.write(CONTENT, &data).unwrap();
    assert_eq!(one.length, two.length);
    assert!(
        one.chunk_hashes()
            .iter()
            .all(|hash| !two.chunk_hashes().contains(hash)),
        "two keys produced a shared ciphertext"
    );

    let shared = MemoryStore::new();
    let reader = Vault::new(shared.clone(), key(5)).with_chunker(ChunkerConfig::SMALL);
    let writer = Vault::new(shared, key(4)).with_chunker(ChunkerConfig::SMALL);
    let manifest = writer.write(CONTENT, &data).unwrap();
    assert!(matches!(reader.read(&manifest), Err(StoreError::Corrupt)));
}
