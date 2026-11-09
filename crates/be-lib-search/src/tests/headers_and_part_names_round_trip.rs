use super::*;

#[test]
fn headers_and_part_names_round_trip() {
    let mut store = Store::default();
    let mut index = Index::default();
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: (0..10).map(|n| put(n, 0, 1)).collect(),
            ..Batch::default()
        },
    );
    let decoded = Index::decode(&index.encode()).unwrap();
    assert_eq!(decoded, index);
    for name in index.parts() {
        assert_eq!(PartName::from_bytes(&name.to_bytes()).unwrap(), name);
        assert!(store.parts.parts.contains_key(&name));
    }
    assert_eq!(found(&decoded, &store, "mark3"), [doc(3)].into());
    assert!(Index::decode(&[0xff; 3]).is_err());
}
