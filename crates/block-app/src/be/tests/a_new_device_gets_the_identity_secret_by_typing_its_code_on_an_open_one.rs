use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

fn pair_in_background(
    harness: &Harness,
    code: String,
) -> std::thread::JoinHandle<Result<[u8; 32], crate::accounts::AccountError>> {
    let (url, account) = (harness.url.clone(), harness.account);
    let token = runtime()
        .block_on(
            be_server::ServerStore::open(harness.directory.join("server"))
                .unwrap()
                .issue_session(account),
        )
        .expect("the new device signs in");
    std::thread::spawn(move || {
        let (_cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        runtime().block_on(crate::accounts::pair(
            url,
            token,
            account,
            code,
            "test".to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        ))
    })
}

#[test]
fn a_new_device_gets_the_identity_secret_by_typing_its_code_on_an_open_one() {
    let harness = Harness::start();
    harness.connect();
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(crate::be::Query::Roots)
    });

    let mistyped = pair_in_background(&harness, be_keys::pairing_code());
    wait_until("heard the new device", |shared| !shared.pairing.is_empty());
    let request = pairing_requests().remove(0);
    assert_eq!(request.device, "test");
    approve_pairing(request.from, be_keys::pairing_code());
    assert!(
        mistyped.join().unwrap().is_err(),
        "a wrong code still handed over the identity secret"
    );

    let code = be_keys::pairing_code();
    let paired = pair_in_background(&harness, code.clone());
    wait_until("heard the new device again", |shared| {
        !shared.pairing.is_empty()
    });
    let request = pairing_requests().remove(0);
    approve_pairing(request.from, code.to_lowercase());
    let secret = paired.join().unwrap().expect("the right code pairs");
    assert_eq!(
        secret,
        harness.identity(),
        "the new device did not get the account's identity secret"
    );

    let keys = runtime()
        .block_on(crate::accounts::keys(
            harness.url.clone(),
            harness.token.clone(),
        ))
        .expect("the keys load");
    let mut state = crate::keys::KeyState::default();
    state.loaded(keys);
    assert!(state.needs_unlock(None));
    assert!(
        !state.needs_unlock(Some(secret)),
        "the paired secret does not match the account's recovery key"
    );
}
