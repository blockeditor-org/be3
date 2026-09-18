use super::*;

#[test]
fn a_ranged_read_only_fetches_overlapping_chunks() {
    let store = CountingStore::default();
    let vault = Vault::new(store.clone(), key(3)).with_chunker(ChunkerConfig::SMALL);
    let data = pseudorandom(128 * 1024, 17);
    let manifest = vault.write(CONTENT, &data).unwrap();
    assert!(manifest.chunks.len() > 32);

    let before = store.reads();
    let slice = vault.read_range(&manifest, 70_000, 900).unwrap();
    let fetched = store.reads() - before;
    assert_eq!(slice, data[70_000..70_900]);
    assert!(
        fetched <= 3,
        "a 900 byte read fetched {fetched} of {} chunks",
        manifest.chunks.len()
    );

    assert_eq!(vault.read_range(&manifest, 0, 10).unwrap(), data[..10]);
    assert_eq!(
        vault
            .read_range(&manifest, manifest.length - 5, 100)
            .unwrap(),
        data[data.len() - 5..]
    );
    assert!(
        vault
            .read_range(&manifest, manifest.length, 10)
            .unwrap()
            .is_empty()
    );
}
