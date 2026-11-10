use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn an_endpoint_that_keeps_failing_for_a_day_is_dropped() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut member = harness.client().await;
    let account = member.register("member@example.com").await;
    let organization = member.personal().await;
    let address = "http://127.0.0.1:9/broken".to_owned();
    member
        .register_endpoint(TestDevice::new().endpoint(PushService::WebPush, address.clone()))
        .await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let kept = |store: ServerStore| async move {
        store
            .push_endpoints(account, organization)
            .await
            .unwrap()
            .len()
    };

    for _ in 0..push::MAX_FAILURES {
        store
            .endpoint_failed(account, organization, address.clone())
            .await
            .unwrap();
    }
    assert_eq!(
        kept(store.clone()).await,
        1,
        "failures within a day keep the endpoint: the device may be away"
    );
    store
        .endpoint_answered(account, organization, address.clone())
        .await
        .unwrap();
    for _ in 0..push::MAX_FAILURES {
        store
            .endpoint_failed(account, organization, address.clone())
            .await
            .unwrap();
    }
    let moved = address.clone();
    store
        .with_database(move |_, database| {
            database.execute(
                "UPDATE push_endpoints SET failing_since = failing_since - ?2 WHERE address = ?1",
                rusqlite::params![moved, i64::try_from(push::FAILING_FOR.as_millis()).unwrap()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(kept(store.clone()).await, 1);
    store
        .endpoint_failed(account, organization, address)
        .await
        .unwrap();
    assert_eq!(
        kept(store).await,
        0,
        "an endpoint failing for a day is dropped"
    );

    harness.stop().await;
}
