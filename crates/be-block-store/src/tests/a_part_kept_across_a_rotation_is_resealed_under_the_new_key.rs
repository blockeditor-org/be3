use super::*;

#[test]
fn a_part_kept_across_a_rotation_is_resealed_under_the_new_key() {
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
    let kept = || {
        Part::node(vec![
            Part::from(&header),
            Part::Reused(loaded.children[1].reference),
        ])
    };
    assert!(vault.stale_parts(id(1), &kept()).unwrap().is_empty());
    assert_eq!(
        vault.write(id(1), CONTENT, kept()).unwrap(),
        first,
        "under the same key a kept part is reused as it is"
    );

    keys.insert(id(2), ContentKey::from_bytes([2; 32]));
    assert_eq!(
        vault.stale_parts(id(2), &kept()).unwrap(),
        [loaded.children[1].reference]
    );
    let rekeyed = vault.write(id(2), CONTENT, kept()).unwrap();
    assert_ne!(rekeyed, first);
    let mut sealed_under = std::collections::BTreeSet::new();
    for hash in vault.objects(&rekeyed).unwrap() {
        let sealed = vault.store().get(hash).unwrap().unwrap();
        sealed_under.insert(sealed_key(&sealed).unwrap());
    }
    assert_eq!(
        sealed_under,
        [id(2)].into(),
        "nothing stays under the old key"
    );

    let without_old = Keyring::new();
    without_old.insert(id(2), ContentKey::from_bytes([2; 32]));
    let reader = Vault::new(vault.store().clone(), without_old);
    assert_eq!(reader.read(&rekeyed).unwrap(), [header, payload].concat());
}
