use super::*;

#[test]
fn workspace_keys_belong_to_their_account_and_leave_with_it() {
    let store = AppStateStore::open(":memory:").unwrap();
    let mine = account(ServerLocation::Local, "mine@example.com");
    let theirs = account(
        ServerLocation::Remote("https://notes.example.com".into()),
        "theirs@example.com",
    );
    store.save_account(&mine).unwrap();
    store.save_account(&theirs).unwrap();
    let workspace = Uuid::new_v4();

    assert_eq!(store.workspace_key(&mine, workspace).unwrap(), None);
    store.set_workspace_key(&mine, workspace, [1; 32]).unwrap();
    store.set_workspace_key(&mine, workspace, [2; 32]).unwrap();
    store
        .set_workspace_key(&theirs, workspace, [3; 32])
        .unwrap();
    assert_eq!(
        store.workspace_key(&mine, workspace).unwrap(),
        Some([2; 32])
    );
    assert_eq!(
        store.workspace_keys(&mine).unwrap(),
        vec![(workspace, [2; 32])]
    );

    store.remove_account(&mine).unwrap();
    assert_eq!(store.workspace_key(&mine, workspace).unwrap(), None);
    assert_eq!(
        store.workspace_key(&theirs, workspace).unwrap(),
        Some([3; 32])
    );
}
