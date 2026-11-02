use super::*;

#[test]
fn fresh_store_has_no_accounts() {
    let store = HostStateStore::open(":memory:").unwrap();
    assert!(store.accounts().unwrap().is_empty());
    assert_eq!(store.active_account().unwrap(), None);
}
