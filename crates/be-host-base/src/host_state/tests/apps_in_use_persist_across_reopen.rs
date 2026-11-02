use super::*;

#[test]
fn apps_in_use_persist_across_reopen() {
    let path = std::env::temp_dir().join(format!("be-host-state-{}.sqlite3", Uuid::new_v4()));
    {
        let store = HostStateStore::open(&path).unwrap();
        assert!(store.warm_apps().unwrap().is_empty());
        store
            .set_warm_apps(&["be3.database".to_owned(), "be3.workspace-ui".to_owned()])
            .unwrap();
    }
    let store = HostStateStore::open(&path).unwrap();
    assert_eq!(
        store.warm_apps().unwrap(),
        ["be3.database", "be3.workspace-ui"]
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
