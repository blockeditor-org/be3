use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

#[test]
fn an_account_unlocks_with_its_recovery_phrase() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    runtime().block_on(async {
        let peer = harness.outside_peer().await;
        peer.ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .unwrap();
        let mut counter = CounterContent::default();
        counter.apply(&Counter::add(5));
        peer.save(block, &counter, None).await.unwrap();
    });

    let keys = runtime()
        .block_on(crate::accounts::keys(
            harness.url.clone(),
            harness.token.clone(),
        ))
        .expect("the keys load");
    let mut state = crate::keys::KeyState::default();
    state.loaded(keys);
    assert!(!state.needs_recovery());
    assert!(
        state.needs_unlock(None),
        "a device without the account's secret was not asked to unlock"
    );
    let stranger = be_keys::RecoveryPhrase::generate().secret().to_bytes();
    assert!(state.needs_unlock(Some(stranger)));
    assert_eq!(
        state.unlock_with_phrase(&be_keys::RecoveryPhrase::generate().to_string()),
        None,
        "another account's phrase unlocked this one"
    );
    let secret = state
        .unlock_with_phrase(&harness.phrase.to_string())
        .expect("the account's own phrase unlocks it");
    assert!(!state.needs_unlock(Some(secret)));

    let elsewhere = std::env::temp_dir().join(format!("block-app-be-unlocked-{}", Uuid::new_v4()));
    start(Config {
        identity: secret,
        ..harness.config(elsewhere.clone())
    });
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 5);
    stop_and_wait();
    let _ = std::fs::remove_dir_all(&elsewhere);
}
