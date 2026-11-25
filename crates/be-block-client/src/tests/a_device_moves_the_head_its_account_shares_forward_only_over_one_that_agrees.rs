use super::*;

async fn append(peer: &Peer<MemoryStore>, block: Uuid, texts: &[&str]) {
    for text in texts {
        peer.append(
            block,
            "test.app",
            text.as_bytes(),
            be_block_content::Along::default(),
        )
        .await
        .unwrap();
    }
    peer.read_log(block, 0, 10).await.unwrap();
}

/// A device moves the head the account's block holds for a block forward only over one it
/// compared and found agreeing (guides/blocks/keys.md, "Forks"): a longer head of the same log
/// replaces a shorter one, a shorter never replaces a longer, and one the comparison could not
/// settle stays, here as the server answered a proof that does not check.
#[tokio::test]
async fn a_device_moves_the_head_its_account_shares_forward_only_over_one_that_agrees() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let organization = laptop.organization();
    let chat = workspace(&laptop, "Chat").await;
    append(&laptop, chat, &["one"]).await;
    laptop.trade_account_heads().await.unwrap();
    let phone = another(&harness, &laptop).await;
    append(&phone, chat, &["two"]).await;
    let compared = phone.trade_account_heads().await.unwrap();
    assert!(
        matches!(compared.as_slice(), [(block, Ok(LogAgreement::Agrees))] if *block == chat),
        "{compared:?}"
    );
    assert_eq!(
        phone
            .account_log_head(organization, chat)
            .await
            .unwrap()
            .size,
        2,
        "a longer head of the same log moves the shared one forward"
    );
    // The laptop kept the shorter head: it agrees, and shares nothing over the longer one.
    let compared = laptop.trade_account_heads().await.unwrap();
    assert!(
        matches!(compared.as_slice(), [(_, Ok(LogAgreement::Agrees))]),
        "{compared:?}"
    );
    let held = phone.account_log_head(organization, chat).await.unwrap();
    assert_eq!(held.size, 2, "a shorter head never replaces a longer");

    // The phone reads on, then the server's tree loses the node over its first two leaves to a
    // made-up hash, so the root it names at the shared head's size does not prove against the
    // head the phone kept.
    append(&phone, chat, &["three", "four"]).await;
    harness
        .database()
        .execute(
            "UPDATE block_log_nodes SET hash = ?1
             WHERE organization_id = ?2 AND block_id = ?3 AND level = 1 AND position = 0",
            rusqlite::params![vec![9u8; 32], organization.to_string(), chat.to_string()],
        )
        .unwrap();
    let compared = phone.trade_account_heads().await.unwrap();
    assert!(
        matches!(compared.as_slice(), [(block, Err(ClientError::Tampered(_)))] if *block == chat),
        "{compared:?}"
    );
    assert_eq!(
        phone.account_log_head(organization, chat).await,
        Some(held),
        "a head that could not be compared is not moved forward over"
    );
    harness.stop().await;
}
