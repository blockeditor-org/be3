use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

#[test]
fn an_invited_member_is_sent_the_key_by_a_member_who_is_online() {
    let harness = Harness::start();
    harness.connect();
    wait_until("loaded the graph", |shared| shared.graph.loaded);

    let url = harness.url.clone();
    let guest = runtime()
        .block_on(crate::accounts::register(
            url.clone(),
            "guest@example.com".to_owned(),
            "Guest".to_owned(),
            "correct horse battery".to_owned(),
        ))
        .expect("the guest registers");
    runtime()
        .block_on(crate::accounts::invite(
            url.clone(),
            harness.token.clone(),
            harness.workspace,
            "guest@example.com".to_owned(),
            be_protocol::WorkspaceRole::Editor,
        ))
        .expect("the owner invites the guest");
    let (_, invitations) = runtime()
        .block_on(crate::accounts::workspaces(
            url.clone(),
            guest.token.clone(),
        ))
        .expect("the guest lists invitations");
    runtime()
        .block_on(crate::accounts::respond_invitation(
            url.clone(),
            guest.token.clone(),
            invitations[0].id,
            true,
        ))
        .expect("the guest accepts");

    let (_cancel, cancelled) = tokio::sync::oneshot::channel::<()>();
    let waiting = {
        let (url, token, workspace) = (url.clone(), guest.token.clone(), harness.workspace);
        std::thread::spawn(move || {
            runtime().block_on(crate::accounts::sealed_key(
                url,
                token,
                workspace,
                Box::pin(async move {
                    let _ = cancelled.await;
                }),
            ))
        })
    };
    let phrase = be_keys::RecoveryPhrase::generate();
    runtime()
        .block_on(crate::accounts::set_recovery_key(
            url,
            guest.token.clone(),
            phrase.secret().public(),
            Vec::new(),
        ))
        .expect("the guest saves a recovery phrase");

    let sealed = waiting
        .join()
        .unwrap()
        .expect("the owner's open device sealed the key to the guest");
    assert_eq!(
        phrase.secret().open(&sealed).unwrap(),
        harness.content_key,
        "the guest's recovery phrase does not open the workspace key"
    );
}
