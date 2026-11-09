use super::*;

#[test]
fn a_part_kept_across_a_rotation_stays_sealed_under_its_key() {
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
    assert_eq!(
        vault.write(id(1), CONTENT, kept()).unwrap(),
        first,
        "under the same key a kept part is reused as it is"
    );

    keys.insert(id(2), ContentKey::from_bytes([2; 32]));
    let rekeyed = vault.write(id(2), CONTENT, kept()).unwrap();
    assert_ne!(rekeyed, first);
    let payload_objects: std::collections::BTreeSet<_> = vault
        .objects(&Manifest {
            content_type: CONTENT,
            root: Some(loaded.children[1].reference),
        })
        .unwrap()
        .into_iter()
        .collect();
    for hash in vault.objects(&rekeyed).unwrap() {
        let sealed = vault.store().get(hash).unwrap().unwrap();
        let expected = match payload_objects.contains(&hash) {
            true => id(1),
            false => id(2),
        };
        assert_eq!(
            sealed_key(&sealed).unwrap(),
            expected,
            "only what the write made new is sealed under the new key"
        );
        assert_eq!(vault.sealed_under(hash).unwrap(), expected);
    }

    let without_old = Keyring::new();
    without_old.insert(id(2), ContentKey::from_bytes([2; 32]));
    let reader = Vault::new(vault.store().clone(), without_old);
    assert!(
        reader.read(&rekeyed).is_err(),
        "the kept part needs the key it was sealed under"
    );
    let both = Keyring::new();
    both.insert(id(1), ContentKey::from_bytes([1; 32]));
    both.insert(id(2), ContentKey::from_bytes([2; 32]));
    let reader = Vault::new(vault.store().clone(), both);
    assert_eq!(reader.read(&rekeyed).unwrap(), [header, payload].concat());
}
