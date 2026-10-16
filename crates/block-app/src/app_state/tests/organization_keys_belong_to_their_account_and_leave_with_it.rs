use super::*;

#[test]
fn organization_keys_belong_to_their_account_and_leave_with_it() {
    let store = AppStateStore::open(":memory:").unwrap();
    let mine = account(ServerLocation::Local, "mine@example.com");
    let theirs = account(
        ServerLocation::Remote("https://notes.example.com".into()),
        "theirs@example.com",
    );
    store.save_account(&mine).unwrap();
    store.save_account(&theirs).unwrap();
    let organization = Uuid::new_v4();

    assert_eq!(store.organization_key(&mine, organization).unwrap(), None);
    store
        .set_organization_key(&mine, organization, [1; 32])
        .unwrap();
    store
        .set_organization_key(&mine, organization, [2; 32])
        .unwrap();
    store
        .set_organization_key(&theirs, organization, [3; 32])
        .unwrap();
    assert_eq!(
        store.organization_key(&mine, organization).unwrap(),
        Some([2; 32])
    );
    assert_eq!(
        store.organization_keys(&mine).unwrap(),
        vec![(organization, [2; 32])]
    );

    store.remove_account(&mine).unwrap();
    assert_eq!(store.organization_key(&mine, organization).unwrap(), None);
    assert_eq!(
        store.organization_key(&theirs, organization).unwrap(),
        Some([3; 32])
    );
}
