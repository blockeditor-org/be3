use super::*;
use be_block_protocol::SealedKey;

async fn recovery(client: &mut TestClient) -> (Option<[u8; 32]>, Vec<SealedKey>) {
    let response = client
        .call(|peer| client::keys::get_recovery_key(peer))
        .await;
    let Ok(be_block_protocol::RecoveryKey { public, sealed, .. }) = response else {
        panic!("reading the recovery key failed: {response:?}");
    };
    (public, sealed)
}

#[tokio::test]
async fn a_new_recovery_key_reseals_every_key_sealed_to_the_account() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.organization("notes").await;
    assert_eq!(
        recovery(&mut owner).await,
        (Some(test_secret(account).public().0), Vec::new())
    );

    let first = owner.new_phrase(account).await;
    let identity = first.identity();
    let workspace = owner.scope_root(account, BlockParent::Root).await;
    let (public, sealed) = recovery(&mut owner).await;
    assert_eq!(public, Some(identity.public.0));
    assert_eq!(
        sealed,
        vec![SealedKey {
            organization,
            key: workspace.scope,
            sender: account,
            sealed: b"scope sealed to its author".to_vec(),
        }]
    );

    let response = owner
        .call(|peer| {
            client::keys::put_keys(
                peer,
                KeySet {
                    keys: Vec::new(),
                    wrapped: vec![wrap(
                        workspace.scope,
                        WrappedBy::Account(account),
                        b"a forgery",
                    )],
                },
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(
        recovery(&mut owner).await.1[0].sealed,
        b"scope sealed to its author",
        "a second put overwrote a sealed key"
    );

    let refused = owner
        .call(|peer| {
            client::keys::set_recovery_key(
                peer,
                [2; 32],
                [2; 32],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(refused, Err(_)),
        "a new recovery key left a key sealed to the old one: {refused:?}"
    );
    assert_eq!(recovery(&mut owner).await.0, Some(identity.public.0));

    let second = be_block_keys::RecoveryPhrase::generate().secret();
    let next = second.identity();
    let logs = owner
        .call(|peer| client::keys::get_recovery_key(peer))
        .await
        .unwrap()
        .logs;
    assert_eq!(logs.len(), 2, "the organization's log and the workspace's");
    let retirement = |log: be_block_protocol::SignedLog| be_block_protocol::Retirement {
        organization: log.organization,
        kind: log.kind,
        log: log.log,
        entry: be_block_keys::Entry::retirement(
            &first,
            &second,
            account.into_bytes(),
            log.log.into_bytes(),
            log.head.try_into().unwrap(),
        )
        .encode(),
    };
    let mut retirements: Vec<_> = logs.into_iter().map(retirement).collect();
    let kept = retirements.pop().unwrap();
    let response = owner
        .call(|peer| {
            client::keys::set_recovery_key(
                peer,
                next.public.0,
                next.signing.0,
                vec![SealedKey {
                    organization,
                    key: workspace.scope,
                    sender: account,
                    sealed: b"resealed to the new phrase".to_vec(),
                }],
                Vec::new(),
                Vec::new(),
                retirements.clone(),
            )
        })
        .await;
    assert!(
        matches!(response, Err(_)),
        "a new recovery key left a log the old one signed in unretired: {response:?}"
    );
    retirements.push(kept);

    let response = owner
        .call(|peer| {
            client::keys::set_recovery_key(
                peer,
                next.public.0,
                next.signing.0,
                vec![SealedKey {
                    organization,
                    key: workspace.scope,
                    sender: account,
                    sealed: b"resealed to the new phrase".to_vec(),
                }],
                Vec::new(),
                Vec::new(),
                retirements,
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    remember_secret(account, second);
    assert_eq!(
        recovery(&mut owner).await,
        (
            Some(next.public.0),
            vec![SealedKey {
                organization,
                key: workspace.scope,
                sender: account,
                sealed: b"resealed to the new phrase".to_vec(),
            }]
        )
    );

    harness.stop().await;
}
