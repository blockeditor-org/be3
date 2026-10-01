use super::*;

#[test]
fn a_truncated_object_reads_as_missing_and_a_put_repairs_it() {
    let root = std::env::temp_dir().join(format!("be-store-test-{}", Uuid::new_v4()));
    let store = FileStore::open(&root).unwrap();
    let bytes = pseudorandom(4_000, 7);
    let hash = store.put(&bytes).unwrap();
    let hex = hash.to_hex();
    let path = root.join(&hex[..2]).join(&hex[2..]);

    std::fs::write(&path, &bytes[..100]).unwrap();
    assert_eq!(store.put(&bytes).unwrap(), hash);
    assert_eq!(store.get(hash).unwrap(), Some(bytes.clone()));

    let mut flipped = bytes.clone();
    flipped[0] ^= 1;
    std::fs::write(&path, &flipped).unwrap();
    assert_eq!(store.get(hash).unwrap(), None);
    assert!(!store.has(hash).unwrap());
    store.put(&bytes).unwrap();
    assert_eq!(store.get(hash).unwrap(), Some(bytes));

    std::fs::remove_dir_all(&root).unwrap();
}
