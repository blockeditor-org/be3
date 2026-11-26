use super::*;

/// When the server's proof that the root it names at another device's size is a prefix of the
/// head it serves does not check, the server misbehaved, but no fork is shown: the comparison
/// answers `Tampered` and keeps that as the block's verdict of its own kind
/// (`ForkKind::Misbehaved`), never as a fork.
#[tokio::test]
async fn a_prefix_root_whose_proof_does_not_check_is_the_server_misbehaving() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let chat = workspace(&laptop, "Chat").await;
    let append = |text: &'static str| {
        let laptop = &laptop;
        async move {
            laptop
                .append(
                    chat,
                    "test.app",
                    text.as_bytes(),
                    be_block_content::Along::default(),
                )
                .await
                .unwrap();
        }
    };
    append("one").await;
    laptop.read_log(chat, 0, 10).await.unwrap();
    let first = laptop.signed_log_head(chat).unwrap();
    for text in ["two", "three", "four"] {
        append(text).await;
    }
    let phone = another(&harness, &laptop).await;
    assert_eq!(phone.read_log(chat, 0, 10).await.unwrap().head.size, 4);

    // The server's tree loses its first leaf to a made-up hash: the root it names at size 1 no
    // longer proves against the head of size 4, whose node it keeps whole.
    harness
        .database()
        .execute(
            "UPDATE block_log_nodes SET hash = ?1
             WHERE organization_id = ?2 AND block_id = ?3 AND level = 0 AND position = 0",
            rusqlite::params![
                vec![9u8; 32],
                laptop.organization().to_string(),
                chat.to_string()
            ],
        )
        .unwrap();
    assert!(
        matches!(
            phone.compare_log_head(chat, first, HeadVia::Session).await,
            Err(ClientError::Tampered(_))
        ),
        "a proof that does not check was taken"
    );
    assert!(
        matches!(
            phone.log_fork(chat),
            Some(fork) if fork.kind == crate::ForkKind::Misbehaved && fork.theirs.size == 1
        ),
        "the misbehaving is kept as a verdict of its own, not a fork"
    );
    harness.stop().await;
}
