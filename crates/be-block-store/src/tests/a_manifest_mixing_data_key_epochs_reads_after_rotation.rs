use super::*;

#[test]
fn a_manifest_mixing_data_key_epochs_reads_after_rotation() {
    let keys = Keyring::new();
    keys.insert(id(1), ContentKey::from_bytes([1; 32]));
    let vault = Vault::new(MemoryStore::new(), keys.clone()).with_chunker(ChunkerConfig::SMALL);
    let header = pseudorandom(40, 1);
    let payload = pseudorandom(20_000, 2);
    let first = vault
        .write(
            id(1),
            CONTENT,
            Part::node(vec![Part::from(&header), Part::from(&payload)]),
        )
        .unwrap();
    let loaded = vault.load(&first).unwrap().root.unwrap();

    keys.insert(id(2), ContentKey::from_bytes([2; 32]));
    let renamed = pseudorandom(44, 3);
    let second = vault
        .write(
            id(2),
            CONTENT,
            Part::node(vec![
                Part::from(&renamed),
                Part::Reused(loaded.children[1].reference),
            ]),
        )
        .unwrap();
    let mut sealed_under = std::collections::BTreeSet::new();
    for hash in vault.objects(&second).unwrap() {
        let sealed = vault.store().get(hash).unwrap().unwrap();
        sealed_under.insert(sealed_key(&sealed).unwrap());
    }
    assert_eq!(sealed_under, [id(1), id(2)].into());
    assert_eq!(vault.read(&second).unwrap(), [renamed, payload].concat());

    let without_old = Keyring::new();
    without_old.insert(id(2), ContentKey::from_bytes([2; 32]));
    let reader = Vault::new(vault.store().clone(), without_old);
    assert!(matches!(
        reader.read(&second),
        Err(StoreError::MissingKey(missing)) if missing == id(1)
    ));
}
