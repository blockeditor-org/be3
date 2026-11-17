use super::*;

/// An administrator takes a new phrase: what they sealed to the server by the old one, the
/// settings' keys, is sealed again, so the server still reads the policies and takes a block
/// made after it.
#[tokio::test]
async fn an_administrators_new_phrase_leaves_the_server_reading_the_policies() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let fresh = new_phrase(
        &admin.connection(),
        admin.account(),
        &admin.keys().identity().clone(),
    )
    .await;
    let device = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh,
            Credentials::Token(admin.token().to_owned()),
        )
        .organization(Some(admin.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let block = note(&device).await;
    device
        .save(
            block,
            &TextBlock::of("after the new phrase"),
            None,
        )
        .await
        .expect("the server reads the policies a save is kept by");

    harness.stop().await;
}
