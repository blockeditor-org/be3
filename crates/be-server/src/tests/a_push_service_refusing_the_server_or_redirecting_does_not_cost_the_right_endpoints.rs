use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn a_push_service_refusing_the_server_or_redirecting_does_not_cost_the_right_endpoints() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut service = TestPushService::start(|path| match path {
        "/redirect" => (302, String::new()),
        "/denied" => (403, String::new()),
        "/unauthorized" => (401, String::new()),
        _ => (201, String::new()),
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut member, account) = viewer_of(&harness, &mut owner, block, "member@example.com").await;
    for path in ["/redirect", "/denied", "/unauthorized", "/kept"] {
        member
            .register_endpoint(
                TestDevice::new().endpoint(PushService::WebPush, format!("{}{path}", service.url)),
            )
            .await;
    }
    member.sign_out().await;

    owner.notify(block, b"hello", Vec::new()).await;
    // An account's pushes go one at a time in registration order, so once the last arrives
    // the outcome of each before it is recorded.
    for path in ["/redirect", "/denied", "/unauthorized", "/kept"] {
        assert_eq!(service.next().await.path, path);
    }
    let store = ServerStore::open(&harness.directory).unwrap();
    let failures = store
        .with_database(move |_, database| {
            let mut statement = database.prepare(
                "SELECT address, failures FROM push_endpoints WHERE account_id = ?1",
            )?;
            let rows = statement
                .query_map(rusqlite::params![account.to_string()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    let failed = |path: &str| {
        failures
            .iter()
            .find(|(address, _)| address.ends_with(path))
            .map(|(_, failures)| *failures)
    };
    assert_eq!(failed("/redirect"), Some(1), "a redirect is not a delivery");
    assert_eq!(failed("/denied"), Some(0), "a 403 is this server's fault");
    assert_eq!(failed("/unauthorized"), Some(0), "a 401 is this server's fault");
    assert_eq!(failed("/kept"), Some(0));
    let _ = organization;

    harness.stop().await;
}
