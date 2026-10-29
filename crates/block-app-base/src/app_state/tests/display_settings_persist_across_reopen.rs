use super::*;

#[test]
fn display_settings_persist_across_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let content = vec![4, 5, 6, 0, 255];
    {
        let store = AppStateStore::open(&path).unwrap();
        assert_eq!(store.display_settings().unwrap(), None);
        store.set_display_settings(&content).unwrap();
        store.set_display_settings(&content[1..]).unwrap();
        assert_eq!(
            store.input_settings().unwrap(),
            None,
            "display and input settings are kept apart"
        );
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(
        store.display_settings().unwrap(),
        Some(content[1..].to_vec())
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
