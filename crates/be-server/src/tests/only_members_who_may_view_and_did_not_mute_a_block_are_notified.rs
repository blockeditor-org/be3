use super::*;
use be_block_protocol::{NotifyMode, PushService};

#[tokio::test]
async fn only_members_who_may_view_and_did_not_mute_a_block_are_notified() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let endpoint = |name: &str| {
        TestDevice::new().endpoint(PushService::WebPush, format!("http://127.0.0.1:9/{name}"))
    };
    owner.register_endpoint(endpoint("owner")).await;

    let mut accounts = Vec::new();
    for (name, mode) in [
        ("all", NotifyMode::All),
        ("mentions", NotifyMode::Mentions),
        ("off", NotifyMode::Off),
    ] {
        let (mut member, account) =
            viewer_of(&harness, &mut owner, block, &format!("{name}@example.com")).await;
        member.register_endpoint(endpoint(name)).await;
        member.set_notify_mode(block, mode).await;
        let read = member
            .call(|peer| client::notifications::notify_mode_of(peer, block))
            .await;
        assert_eq!(read, Ok(mode));
        accounts.push(account);
    }
    let mut stranger = harness.client().await;
    let stranger_id = stranger.register("stranger@example.com").await;
    stranger.personal().await;
    stranger.register_endpoint(endpoint("stranger")).await;
    let muted = stranger
        .call(|peer| client::notifications::set_notify_mode(peer, block, NotifyMode::Off))
        .await;
    assert!(
        refused_as(&muted, ErrorCode::BlockNotFound),
        "a stranger set a mode on a block it cannot see: {muted:?}"
    );

    let store = ServerStore::open(&harness.directory).unwrap();
    let identity = Identity {
        account: owner_id,
        organization,
        role: OrganizationRole::Editor,
        bound: None,
    };
    let recipients = |mentions: Vec<Uuid>| {
        let store = store.clone();
        async move {
            let mut addresses: Vec<String> = store
                .notification_recipients(identity, block, mentions)
                .await
                .unwrap()
                .into_iter()
                .flat_map(|recipient| recipient.endpoints)
                .map(|endpoint| endpoint.address.rsplit('/').next().unwrap().to_owned())
                .collect();
            addresses.sort();
            addresses
        }
    };
    assert_eq!(recipients(Vec::new()).await, ["all"]);
    assert_eq!(
        recipients(vec![accounts[1], accounts[2], stranger_id]).await,
        ["all", "mentions"]
    );

    harness.stop().await;
}
