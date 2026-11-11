use super::*;
use be_block_protocol::SigningKey;

async fn signing_keys(client: &mut TestClient, accounts: Vec<Uuid>) -> Vec<SigningKey> {
    let response = client
        .call(move |peer| client::keys::get_signing_keys(peer, accounts))
        .await;
    let Ok(mut keys) = response else {
        panic!("asking for signing keys failed: {response:?}");
    };
    keys.sort_by_key(|key| key.account);
    keys
}

async fn with_signing_key(client: &mut TestClient, key: [u8; 32]) {
    let response = client
        .call(move |peer| {
            client::keys::set_recovery_key(peer, key, key, Vec::new(), Vec::new(), Vec::new())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
}

#[tokio::test]
async fn signing_keys_are_served_only_for_accounts_the_caller_knows() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let me = owner.register("owner@example.com").await;
    with_signing_key(&mut owner, [1; 32]).await;
    let organization = owner.organization("team").await;
    let block = owner.create_block(BlockParent::Root).await;
    let (agent, _) = owner.block_agent(organization, block).await.unwrap();
    let mut stranger = harness.client().await;
    let stranger_account = stranger.register("stranger@example.com").await;
    with_signing_key(&mut stranger, [9; 32]).await;
    stranger.organization("elsewhere").await;
    let mut expected = vec![
        SigningKey {
            account: me,
            key: [1; 32],
        },
        SigningKey {
            account: agent,
            key: [8; 32],
        },
    ];
    expected.sort_by_key(|key| key.account);
    assert_eq!(
        signing_keys(
            &mut owner,
            vec![me, agent, stranger_account, Uuid::new_v4()]
        )
        .await,
        expected,
        "the caller's own key and a member's, and nothing of an account it does not know"
    );
    assert_eq!(
        signing_keys(&mut stranger, vec![me, agent]).await,
        Vec::new(),
        "an account outside the organization learns none of its members' keys"
    );
    stranger.personal().await;
    assert_eq!(
        signing_keys(&mut stranger, vec![me]).await,
        vec![SigningKey {
            account: me,
            key: [1; 32],
        }],
        "every account is a member of the personal organization"
    );
    harness.stop().await;
}
