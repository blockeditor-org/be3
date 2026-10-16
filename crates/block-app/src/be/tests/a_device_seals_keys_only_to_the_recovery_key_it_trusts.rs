use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

#[test]
fn a_device_seals_keys_only_to_the_recovery_key_it_trusts() {
    let mut harness = Harness::start();
    let swapped = be_keys::RecoveryPhrase::generate().secret().public();
    runtime()
        .block_on(crate::accounts::set_recovery_key(
            harness.url.clone(),
            harness.token.clone(),
            swapped,
            Vec::new(),
        ))
        .expect("a recovery key the device never saw is saved on the server");
    harness.recovery = Some(be_keys::RecoveryPhrase::generate().secret().public().0);
    harness.connect();
    wait_until("loaded the graph", |shared| shared.graph.loaded);

    let keys = runtime()
        .block_on(crate::accounts::keys(
            harness.url.clone(),
            harness.token.clone(),
        ))
        .expect("the keys load");
    assert_eq!(keys.recovery, Some(swapped));
    assert!(
        keys.sealed.is_empty(),
        "the device sealed the workspace key to a recovery key it does not trust"
    );
}
