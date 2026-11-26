use super::*;

/// The accepter removes the link's owner as a contact before the owner's device finished the
/// link: the link goes with the card it left in the owner's mailbox, so nothing is left to finish,
/// and each is left word that the other is no longer a contact.
#[tokio::test]
async fn a_link_accepted_then_unmade_as_a_contact_leaves_no_link_or_card() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let mut other = harness.member("other@example.com").await;
    let (owner_id, other_id) = (owner.id(), other.id());
    let link = owner.link().await;
    other.accept(link, &[]).await.unwrap();
    let database = metadata_database(&harness.directory);
    let count = |sql: &str| -> i64 {
        database
            .query_row(sql, [owner_id.to_string(), other_id.to_string()], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let cards = "SELECT count(*) FROM mailbox WHERE account_id = ?1 AND sender_id = ?2 AND kind = 'accepted'";
    let links = "SELECT count(*) FROM links WHERE owner_id = ?1 AND accepter_id = ?2";
    assert_eq!((count(cards), count(links)), (1, 1));

    other.remove_contact(owner_id).await.unwrap();
    assert_eq!(
        (count(cards), count(links)),
        (0, 0),
        "a link or its card was left to finish"
    );
    // The accepter's record of the owner stays ahead of the removal, so its device keeps the
    // removal with that record.
    for (to, from, kinds) in [
        (owner_id, other_id, &["removed"][..]),
        (other_id, owner_id, &["contact", "removed"][..]),
    ] {
        let letters: Vec<String> = database
            .prepare(
                "SELECT kind FROM mailbox WHERE account_id = ?1 AND sender_id = ?2 ORDER BY id",
            )
            .unwrap()
            .query_map([to.to_string(), from.to_string()], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(letters, kinds, "what each is left of the other");
    }
    let listed = owner.links().await;
    assert!(listed.is_empty(), "{listed:?}");
}
