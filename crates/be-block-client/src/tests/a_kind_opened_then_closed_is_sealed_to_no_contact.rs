use super::*;

use be_block_graph::{Access, PREFIXES_TYPE};
use be_block_protocol::Unsealed;

/// How many wraps the server holds of `key` sealed to `account`.
fn sealed(database: &rusqlite::Connection, key: Uuid, account: Uuid) -> i64 {
    database
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [key.to_string(), format!("account:{account}")],
            |row| row.get(0),
        )
        .unwrap()
}

/// A kind its author opened to anyone inside and closed again, by a signed closing, is
/// offered to no contact the server goes on listing.
#[tokio::test]
async fn a_kind_opened_then_closed_is_sealed_to_no_contact() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&admin, "Team").await;
    admin
        .grant(team, member.account(), Access::View)
        .await
        .unwrap();
    let kind = admin
        .create_block(
            Uuid::now_v7(),
            PREFIXES_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("Prefixes"),
            false,
        )
        .await
        .unwrap()
        .id;
    admin.open_inside(kind, Access::View).await.unwrap();
    let key = admin.keys().scope_key(kind).unwrap();
    admin.open_inside(kind, Access::None).await.unwrap();
    let (_, checked) = admin.checked_access(kind).await.unwrap();
    assert_eq!(checked.open_inside, be_block_keys::Level::None);

    let mut work = admin.key_work().await.unwrap();
    work.rotations.clear();
    work.unsealed.retain(|unsealed| unsealed.key != key);
    work.unsealed.push(Unsealed {
        key,
        account: member.account(),
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&harness.database(), key, member.account()),
        0,
        "a kind its log closed was offered to a contact the server lists"
    );

    harness.stop().await;
}
