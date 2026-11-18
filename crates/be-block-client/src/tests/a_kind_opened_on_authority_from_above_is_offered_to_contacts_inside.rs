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

/// A kind opened by a manager of a block above it, on `Authority::Inherited`, is offered to a
/// contact viewing inside as one its own manager opened: the signed parent records prove the
/// block above is above, and its log, checked from the root they name, the opener's Manage.
#[tokio::test]
async fn a_kind_opened_on_authority_from_above_is_offered_to_contacts_inside() {
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
    let opening = admin
        .access_log(kind)
        .await
        .unwrap()
        .into_iter()
        .find_map(|entry| match entry.record {
            be_block_keys::Record::Opening(opening) => Some(opening.authority),
            _ => None,
        });
    assert!(
        opening.as_ref().and_then(|authority| authority.above()) == Some(team.into_bytes()),
        "the opening rests on the block above: {opening:?}"
    );

    let mut work = admin.key_work().await.unwrap();
    work.unsealed.retain(|unsealed| unsealed.key != key);
    work.unsealed.push(Unsealed {
        key,
        account: contact.account(),
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&harness.database(), key, contact.account()),
        1,
        "a kind opened by a manager from above was not offered to a contact inside"
    );

    harness.stop().await;
}
