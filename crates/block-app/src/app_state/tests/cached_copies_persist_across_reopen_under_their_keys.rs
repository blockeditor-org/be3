use super::*;

#[test]
fn cached_copies_persist_across_reopen_under_their_keys() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let content = vec![4, 5, 6, 0, 255];
    {
        let store = AppStateStore::open(&path).unwrap();
        assert_eq!(store.cached_copy("display settings").unwrap(), None);
        store.cache_copy("display settings", &content).unwrap();
        store.cache_copy("display settings", &content[1..]).unwrap();
        assert_eq!(
            store.cached_copy("input settings").unwrap(),
            None,
            "each key is kept apart"
        );
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(
        store.cached_copy("display settings").unwrap(),
        Some(content[1..].to_vec())
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
