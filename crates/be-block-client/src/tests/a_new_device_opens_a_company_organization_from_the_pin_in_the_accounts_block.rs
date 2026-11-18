use super::*;

/// A member joins a company organization by an invitation, which carries what the
/// administrator pinned for it. The pin arrives in the member's mailbox and is taken into its
/// account's block; a new device of the member, holding only the identity secret, opens the
/// organization with that pin, and the server keeps no pin of its own.
#[tokio::test]
async fn a_new_device_opens_a_company_organization_from_the_pin_in_the_accounts_block() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let escrow = admin.keys().escrow().expect("the administrator made an escrow key");
    let member = harness.member(&admin, "member@example.com").await;
    member.refresh_contacts().await.unwrap();
    assert_eq!(member.keys().pins().escrow, Some(escrow.clone()));

    let letters: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM mailbox WHERE kind = 'pinned'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(letters, 0, "a pin was left in a mailbox once taken");
    let tables: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'pinned_escrows'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0, "the server keeps pins of its own");

    let laptop = harness.second(&member).await;
    assert_eq!(
        laptop.keys().pins(),
        member.keys().pins(),
        "the new device pinned what the account's block holds"
    );
    assert_eq!(laptop.keys().escrow(), Some(escrow));

    harness.stop().await;
}
