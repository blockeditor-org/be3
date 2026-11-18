use super::*;

/// The server keeps every pin a letter brought and sends an older one again, under a new
/// message, after the member's device took a newer one. Neither the account's block nor a new
/// device takes it, since a pin is taken only over an older one, whatever its place in the
/// mailbox.
#[tokio::test]
async fn an_older_pin_the_server_sends_again_is_not_taken() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    member.refresh_contacts().await.unwrap();
    let held = member.keys().pins();
    assert!(held.escrow.is_some(), "the member pinned the escrow key");

    let older = crate::contacts::Pinned {
        root: Some([9; 32]),
        escrow: None,
        made: 1,
    };
    harness
        .database()
        .execute(
            "INSERT INTO mailbox (account_id, sender_id, kind, letter, created)
             VALUES (?1, ?2, 'pinned', ?3, unixepoch())",
            rusqlite::params![
                member.account().to_string(),
                admin.account().to_string(),
                be_block_protocol::encode(&be_block_protocol::Letter::Pinned {
                    organization: admin.organization(),
                    sealed: crate::contacts::seal_pin(
                        member.keys().identity(),
                        admin.organization(),
                        &older,
                    ),
                })
                .unwrap()
            ],
        )
        .unwrap();
    member.refresh_contacts().await.unwrap();

    let kept = crate::contacts::pinned(
        &member.connection(),
        member.account(),
        member.keys().identity(),
        admin.organization(),
    )
    .await
    .unwrap()
    .expect("the member pins the organization");
    assert_ne!(kept.root, older.root, "the block took the older pin");
    let laptop = harness.second(&member).await;
    assert_eq!(
        laptop.keys().pins(),
        held,
        "a new device took the older pin"
    );

    harness.stop().await;
}
