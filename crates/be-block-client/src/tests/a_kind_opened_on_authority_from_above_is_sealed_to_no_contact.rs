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

/// A kind opened by a manager of a block above it, on `Authority::Inherited`, rests on the
/// server's tree until parents are signed: it is offered to no contact (signed access 3c).
#[tokio::test]
async fn a_kind_opened_on_authority_from_above_is_sealed_to_no_contact() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    let team = workspace(&admin, "Team").await;
    let identity = admin.verified_identity(deputy.account()).unwrap();
    admin
        .share(team, deputy.account(), Access::Manage, &identity)
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
    // Granted the kind itself only to view, so the deputy's Manage comes from the block above.
    admin
        .share(kind, deputy.account(), Access::View, &identity)
        .await
        .unwrap();
    let key = admin.keys().scope_key(kind).unwrap();
    deputy.refresh_keys(None).await.unwrap();
    deputy.open_inside(kind, Access::View).await.unwrap();
    let (_, checked) = admin.checked_access(kind).await.unwrap();
    assert_eq!(checked.open_inside, be_block_keys::Level::View);
    assert!(checked.open_inside_tainted, "the opening rests on the block above");

    let mut work = admin.key_work().await.unwrap();
    work.unsealed.retain(|unsealed| unsealed.key != key);
    work.unsealed.push(Unsealed {
        key,
        account: contact.account(),
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&harness.database(), key, contact.account()),
        0,
        "a kind opened on the server's tree was offered to a contact"
    );

    harness.stop().await;
}
