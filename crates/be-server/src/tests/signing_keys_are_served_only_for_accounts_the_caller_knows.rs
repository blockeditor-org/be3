use super::*;
use be_block_protocol::SigningKey;

async fn signing_keys(client: &mut TestClient, accounts: Vec<Uuid>) -> Vec<SigningKey> {
    let response = client
        .call(move |peer| peer.read::<Vec<be_block_protocol::SigningKey>>(be_block_protocol::Query::SigningKeys(accounts)))
        .await;
    let Ok(mut keys) = response else {
        panic!("asking for signing keys failed: {response:?}");
    };
    keys.sort_by_key(|key| key.account);
    keys
}

/// Moves `account` to a new phrase, retiring its old keys in every log it signed in, and
/// answers the new signing key.
async fn with_new_phrase(client: &mut TestClient, account: Uuid) -> [u8; 32] {
    client.new_phrase(account).await.signing().0
}

#[tokio::test]
async fn signing_keys_are_served_only_for_accounts_the_caller_knows() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let me = owner.register("owner@example.com").await;
    let organization = owner.organization("team").await;
    let block = owner.create_block(BlockParent::Root).await;
    let (agent, _) = owner.block_agent(organization, block).await.unwrap();
    let owner_key = with_new_phrase(&mut owner, me).await;
    let mut stranger = harness.client().await;
    let stranger_account = stranger.register("stranger@example.com").await;
    stranger.organization("elsewhere").await;
    with_new_phrase(&mut stranger, stranger_account).await;
    let mut expected = vec![
        SigningKey {
            account: me,
            key: owner_key,
        },
        SigningKey {
            account: agent,
            key: test_secret(agent).signing().0,
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
            key: owner_key,
        }],
        "every account is a member of the personal organization"
    );
    harness.stop().await;
}
