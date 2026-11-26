use super::*;

/// The push keys are made once, by whichever connection gets there first, and keep the endpoint
/// beside them; a push is shown once until it is older than what is forgotten.
#[test]
fn push_keys_are_made_once_and_a_push_is_shown_once() {
    let path = std::env::temp_dir().join(format!("be-host-state-{}.sqlite3", Uuid::new_v4()));
    let store = HostStateStore::open(&path).unwrap();
    let other = HostStateStore::open(&path).unwrap();
    assert_eq!(store.push_keys().unwrap(), None);
    store
        .set_push_endpoint(Some("https://push.example/a"))
        .unwrap();
    assert_eq!(store.push_endpoint().unwrap(), None);

    store.keep_push_keys(&[1; 48]).unwrap();
    other.keep_push_keys(&[2; 48]).unwrap();
    let keys = vec![1; 48];
    assert_eq!(other.push_keys().unwrap(), Some(keys.clone()));
    store
        .set_push_endpoint(Some("https://push.example/a"))
        .unwrap();
    assert_eq!(
        other.push_endpoint().unwrap().as_deref(),
        Some("https://push.example/a")
    );
    other.set_push_endpoint(None).unwrap();
    assert_eq!(store.push_endpoint().unwrap(), None);
    assert_eq!(store.push_keys().unwrap(), Some(keys));

    assert!(store.first_showing(b"one", 10, 0).unwrap());
    assert!(!other.first_showing(b"one", 11, 0).unwrap());
    assert!(store.first_showing(b"two", 12, 0).unwrap());
    // Once forgotten, a push is new again.
    assert!(store.first_showing(b"one", 30, 11).unwrap());
    drop((store, other));
    std::fs::remove_file(path).unwrap();
}
