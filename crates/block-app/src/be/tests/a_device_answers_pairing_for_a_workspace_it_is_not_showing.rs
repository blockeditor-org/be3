use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

#[test]
fn a_device_answers_pairing_for_a_workspace_it_is_not_showing() {
    let harness = Harness::start();
    let other = runtime()
        .block_on(crate::accounts::create_workspace(
            harness.url.clone(),
            harness.token.clone(),
            "Elsewhere".to_owned(),
        ))
        .expect("the second workspace is created");
    let other_key = *be_store::ContentKey::random().as_bytes();
    start(Config {
        other_keys: vec![(other.id, other_key)],
        ..harness.config(harness.directory.join("objects"))
    });
    wait_until("loaded the graph", |shared| shared.graph.loaded);

    let code = be_keys::pairing_code();
    let (url, token, sent) = (harness.url.clone(), harness.token.clone(), code.clone());
    let paired = std::thread::spawn(move || {
        let (_cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
        runtime().block_on(crate::accounts::pair(
            url,
            token,
            other.id,
            sent,
            "test".to_owned(),
            Box::pin(async move {
                let _ = cancelled.await;
            }),
        ))
    });
    wait_until("heard the new device", |shared| !shared.pairing.is_empty());
    let request = pairing_requests().remove(0);
    assert_eq!(request.workspace, other.id);
    approve_pairing(request.from, code);
    assert_eq!(
        paired.join().unwrap().expect("the right code pairs"),
        other_key,
        "the device handed over the key of the workspace it had open instead"
    );
}
