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
    let (url, token, workspace) = (
        harness.url.clone(),
        harness.token.clone(),
        harness.workspace,
    );
    std::thread::spawn(move || {
        let (_cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        runtime().block_on(crate::accounts::pair(
            url,
            token,
            workspace,
            code,
            "test".to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        ))
    })
}

#[test]
fn a_new_device_gets_the_key_by_typing_its_code_on_an_open_one() {
    let harness = Harness::start();
    let phrase = be_keys::RecoveryPhrase::generate();
    runtime()
        .block_on(crate::accounts::set_recovery_key(
            harness.url.clone(),
            harness.token.clone(),
            phrase.secret().public(),
            Vec::new(),
        ))
        .expect("the recovery key is saved");
    harness.connect();
    wait_until("loaded the graph", |shared| shared.graph.loaded);

    let keys = runtime()
        .block_on(crate::accounts::keys(
            harness.url.clone(),
            harness.token.clone(),
        ))
        .expect("the keys load");
    let sealed = keys
        .sealed
        .get(&harness.workspace)
        .expect("the open device sealed the workspace key to the recovery phrase");
    assert_eq!(
        phrase.secret().open(sealed).unwrap(),
        harness.content_key,
        "the recovery phrase does not open the workspace key"
    );

    let mistyped = pair_in_background(&harness, be_keys::pairing_code());
    wait_until("heard the new device", |shared| !shared.pairing.is_empty());
    let request = pairing_requests().remove(0);
    assert_eq!(request.device, "test");
    approve_pairing(request.from, be_keys::pairing_code());
    assert!(
        mistyped.join().unwrap().is_err(),
        "a wrong code still handed over the key"
    );

    let code = be_keys::pairing_code();
    let paired = pair_in_background(&harness, code.clone());
    wait_until("heard the new device again", |shared| {
        !shared.pairing.is_empty()
    });
    let request = pairing_requests().remove(0);
    approve_pairing(request.from, code.to_lowercase());
    assert_eq!(
        paired.join().unwrap().expect("the right code pairs"),
        harness.content_key
    );
}
