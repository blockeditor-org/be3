use super::*;

#[test]
fn a_first_part_loads_without_the_rest() {
    let store = CountingStore::default();
    let vault = Vault::new(store.clone(), keys(9)).with_chunker(ChunkerConfig::SMALL);
    let header = pseudorandom(48, 1);
    let payload = pseudorandom(64 * 1024, 2);
    let manifest = vault
        .write(
            id(9),
            CONTENT,
            Part::node(vec![Part::from(&header), Part::from(&payload)]),
        )
        .unwrap();

    let before = store.reads();
    let stored = vault.load_first(&manifest).unwrap();
    assert_eq!(stored.bytes, header);
    assert_eq!(stored.length(), (header.len() + payload.len()) as u64);
    let root = stored.root.clone().unwrap();
    assert!(stored.holds(&root.children[0]));
    assert!(!stored.holds(&root.children[1]));
    assert!(
        store.reads() - before <= 3,
        "loading the header read {} objects",
        store.reads() - before
    );

    let payload_part = root.children[1].reference;
    assert_eq!(
        vault.read_part(payload_part, 1000, 24).unwrap(),
        payload[1000..1024]
    );

    let whole = vault.write(id(9), CONTENT, &payload).unwrap();
    assert_eq!(vault.load_first(&whole).unwrap().bytes, payload);
}
