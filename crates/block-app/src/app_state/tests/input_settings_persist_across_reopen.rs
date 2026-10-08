use super::*;

#[test]
fn input_settings_persist_across_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let content = vec![1, 2, 3, 0, 255];
    {
        let store = AppStateStore::open(&path).unwrap();
        assert_eq!(store.input_settings().unwrap(), None);
        store.set_input_settings(&content).unwrap();
        store.set_input_settings(&content[1..]).unwrap();
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(store.input_settings().unwrap(), Some(content[1..].to_vec()));
    drop(store);
    std::fs::remove_file(path).unwrap();
}
