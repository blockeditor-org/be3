use super::*;

#[test]
fn the_last_organization_belongs_to_its_account_and_leaves_with_it() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let first = account(ServerLocation::Local, "first@example.com");
    let second = account(ServerLocation::Local, "second@example.com");
    let organization = be_protocol::Organization {
        id: Uuid::new_v4(),
        name: "Acme".into(),
        kind: be_protocol::OrganizationKind::Company,
        role: be_protocol::OrganizationRole::Administrator,
    };
    {
        let store = AppStateStore::open(&path).unwrap();
        store.save_account(&first).unwrap();
        store.save_account(&second).unwrap();
        store
            .set_last_organization_details(&first, &organization)
            .unwrap();
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(store.last_organization(&first).unwrap(), Some(organization));
    assert_eq!(store.last_organization(&second).unwrap(), None);
    store.remove_account(&first).unwrap();
    assert_eq!(store.last_organization(&first).unwrap(), None);
    drop(store);
    std::fs::remove_file(path).unwrap();
}
