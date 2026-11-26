use super::*;

/// Two accounts become contacts by a link, each on one device. A second device of each lists
/// the other as verified, read from the account's own block, which holds the records; the
/// server keeps only that the two are contacts, and once the devices took them nothing of the
/// records is left in the mailboxes it holds.
#[tokio::test]
async fn a_contact_made_on_one_device_is_read_by_another_from_the_accounts_block() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.owner("guest@example.com").await;
    befriend(&rosa, &guest).await;

    for (peer, other) in [(&rosa, &guest), (&guest, &rosa)] {
        let laptop = harness.second(peer).await;
        let listed = laptop.refresh_contacts().await.unwrap();
        let [contact] = &listed[..] else {
            panic!("the second device lists other than one contact: {listed:?}");
        };
        assert_eq!(contact.account, other.account());
        assert_eq!(
            laptop.verified_identity(other.account()),
            Some(other.keys().identity().identity()),
            "the second device verified the contact's keys from the account's block"
        );
    }

    let database = harness.database();
    let columns: Vec<String> = database
        .prepare("SELECT name FROM pragma_table_info('contacts') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        columns,
        ["account_id", "contact_id"],
        "the server keeps more of a contact than that two accounts are contacts"
    );
    let letters: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM mailbox WHERE kind IN ('contact', 'accepted')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(letters, 0, "a record was left in a mailbox once taken");

    harness.stop().await;
}
