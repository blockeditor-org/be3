use super::*;

#[tokio::test]
async fn an_account_quota_refuses_what_would_pass_it_but_not_a_big_file_within() {
    const OBJECT: usize = 1024 * 1024;
    let quota = 3 * OBJECT as u64;
    let harness = Harness::configured(ServerConfig {
        account_quota: Some(quota),
        ..ServerConfig::OPEN
    })
    .await;
    let mut client = harness.client().await;
    client.register("hoarder@example.com").await;
    client.organization("quota").await;
    let object = |seed: u8| -> Vec<u8> {
        (0..OBJECT)
            .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
            .collect()
    };

    let big = vec![object(1), object(2)];
    let stored = client.put(big.clone()).await;
    assert!(stored.is_ok(), "two large objects fit: {stored:?}");
    let held = client.storage_usage().await.unwrap();
    assert_eq!(held.pending, 2 * OBJECT as u64);
    assert_eq!(held.quota, Some(quota));

    let again = client.put(big.clone()).await;
    assert!(
        again.is_ok(),
        "uploading the same objects again costs nothing: {again:?}"
    );
    assert_eq!(client.storage_usage().await.unwrap().pending, held.pending);

    let over = client.reserve(&[object(3), object(4)]).await;
    assert!(
        matches!(&over, Err(Error { code, .. }) if *code == ErrorCode::OverQuota),
        "an upload past the quota is refused before any URL is signed: {over:?}"
    );
    let missing = client
        .call(|peer| client::objects::missing_objects(peer, vec![Hash::of(&object(3))]))
        .await
        .unwrap();
    assert_eq!(
        missing,
        vec![Hash::of(&object(3))],
        "a refused upload wrote nothing"
    );
    assert_eq!(client.storage_usage().await.unwrap().pending, held.pending);

    let block = client.create_block(BlockParent::Root).await;
    let hashes: Vec<Hash> = big.iter().map(|bytes| Hash::of(bytes)).collect();
    let taken = client
        .call(|peer| client::objects::hold_objects(peer, block, hashes, Hold::Lasting))
        .await;
    assert!(taken.is_ok(), "{taken:?}");
    let after = client.storage_usage().await.unwrap();
    assert_eq!(
        (after.held, after.pending),
        (held.pending, 0),
        "a block that takes an upload moves its charge from pending to held"
    );
    assert_eq!(client.read_block(block).await.size, held.pending);

    let fits = client.put(vec![object(5)]).await;
    assert!(fits.is_ok(), "the last object that fits: {fits:?}");

    harness.stop().await;
}
