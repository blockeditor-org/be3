use super::*;

#[test]
fn plugins_in_use_persist_across_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    {
        let store = AppStateStore::open(&path).unwrap();
        assert!(store.warm_plugins().unwrap().is_empty());
        store
            .set_warm_plugins(&["be3.database".to_owned(), "be3.workspace-ui".to_owned()])
            .unwrap();
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(
        store.warm_plugins().unwrap(),
        ["be3.database", "be3.workspace-ui"]
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
