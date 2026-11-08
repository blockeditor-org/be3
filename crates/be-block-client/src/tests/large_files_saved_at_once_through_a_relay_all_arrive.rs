use super::*;

#[tokio::test]
async fn large_files_saved_at_once_through_a_relay_all_arrive() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let email = "large@example.com";
    let author = Peer::connect(
        PeerConfig::new(
            &relay.url(),
            identity(email),
            Credentials::Token(harness.server.register(email, email).await),
        ),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let workspace = workspace(&author, "Large").await;
    let mut photos = Vec::new();
    for seed in 0..4 {
        let block = author
            .create::<ImageContent>(BlockParent::Block(workspace))
            .await
            .unwrap();
        let photo = image(
            &format!("{seed}.png"),
            be_block_protocol::MAX_FRAME_BYTES,
            seed,
        );
        photos.push((block, photo));
    }

    let saved = futures_util::future::join_all(
        photos
            .iter()
            .map(|(block, photo)| author.save(*block, photo, None)),
    )
    .await;
    for saved in saved {
        saved.expect("a large file saved alongside others went through");
    }

    let reader = harness.second(&author).await;
    let wanted: Vec<_> = photos.iter().map(|(block, _)| (*block, None)).collect();
    let read = reader.read_contents(&wanted).await.unwrap();
    for (read, (_, photo)) in read.iter().zip(&photos) {
        assert!(
            read.stored.as_ref().map(|stored| &stored.bytes[..]) == Some(&photo.encode()[..]),
            "a large file read back differs from what was saved"
        );
    }
    harness.stop().await;
}
