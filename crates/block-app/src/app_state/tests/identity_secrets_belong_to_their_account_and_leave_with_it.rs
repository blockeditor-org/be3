use super::*;

#[test]
fn identity_secrets_belong_to_their_account_and_leave_with_it() {
    let store = AppStateStore::open(":memory:").unwrap();
    let mine = account(ServerLocation::Local, "mine@example.com");
    let theirs = account(
        ServerLocation::Remote("https://notes.example.com".into()),
        "theirs@example.com",
    );
    store.save_account(&mine).unwrap();
    store.save_account(&theirs).unwrap();
    let agent = Uuid::new_v4();

    assert_eq!(store.identity(&mine).unwrap(), None);
    store.set_identity(&mine, [1; 32]).unwrap();
    store.set_identity(&mine, [2; 32]).unwrap();
    store.set_identity(&theirs, [3; 32]).unwrap();
    store.trust(&mine, agent, [4; 32]).unwrap();
    assert_eq!(store.identity(&mine).unwrap(), Some([2; 32]));
    assert_eq!(store.trusted_keys(&mine).unwrap(), vec![(agent, [4; 32])]);
    assert!(store.trusted_keys(&theirs).unwrap().is_empty());

    store.remove_account(&mine).unwrap();
    assert_eq!(store.identity(&mine).unwrap(), None);
    assert!(store.trusted_keys(&mine).unwrap().is_empty());
    assert_eq!(store.identity(&theirs).unwrap(), Some([3; 32]));
}
