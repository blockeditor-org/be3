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
            .call(|peer| client::notifications::notify_modes(peer))
            .await
            .unwrap();
        let listed: Vec<_> = read.iter().map(|row| (row.block, row.mode)).collect();
        match mode {
            NotifyMode::All => assert!(listed.is_empty(), "{listed:?}"),
            mode => assert_eq!(listed, [(block, mode)]),
        }
        accounts.push(account);
    }
    // A member of the same organization the block was not shared with.
    let mut bystander = harness.client().await;
    let bystander_id = bystander.register("bystander@example.com").await;
    assert_eq!(bystander.personal().await, organization);
    bystander.register_endpoint(endpoint("bystander")).await;
    let muted = bystander
        .call(|peer| client::notifications::set_notify_mode(peer, block, NotifyMode::Off))
        .await;
    assert!(
        refused_as(&muted, ErrorCode::BlockNotFound),
        "a member set a mode on a block it cannot see: {muted:?}"
    );
    let (mut viewer, _) = viewer_of(&harness, &mut owner, block, "viewer@example.com").await;
    let sealed = b"sealed".to_vec();
    let notified = viewer
        .call(|peer| client::notifications::notify(peer, block, sealed, Vec::new(), Vec::new()))
        .await;
    assert!(
        refused_as(&notified, ErrorCode::PermissionDenied),
        "a member who may only view the block notified about it: {notified:?}"
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
                .map(|(endpoint, _)| endpoint.address.rsplit('/').next().unwrap().to_owned())
                .collect();
            addresses.sort();
            addresses
        }
    };
    // The viewer added last registered no endpoint, so only "all" and "mentions" have one.
    assert_eq!(recipients(Vec::new()).await, ["all"]);
    assert_eq!(
        recipients(vec![accounts[1], accounts[2], bystander_id]).await,
        ["all", "mentions"]
    );

    harness.stop().await;
}
