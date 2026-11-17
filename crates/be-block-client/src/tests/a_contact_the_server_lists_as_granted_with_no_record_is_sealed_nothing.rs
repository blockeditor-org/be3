use super::*;

use be_block_graph::Access;
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

/// A device seals a block's scope key only to those its log names, with the keys their grant
/// names (signed access 3c): a verified contact the server lists as due the key, with no grant
/// in the log, is sealed nothing; once a manager's signed grant names them, the same listing
/// seals it to them.
#[tokio::test]
async fn a_contact_the_server_lists_as_granted_with_no_record_is_sealed_nothing() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&admin, "Team").await;
    let scope = admin.keys().scope_key(team).unwrap();
    assert!(
        admin.verified_identity(member.account()).is_some(),
        "the member is a verified contact"
    );
    let database = harness.database();

    let mut work = admin.key_work().await.unwrap();
    work.unsealed.push(Unsealed {
        key: scope,
        account: member.account(),
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&database, scope, member.account()),
        0,
        "a contact the log grants nothing was sealed the key"
    );

    // A grant signed into the log, with the key sealed by its own path withheld.
    admin
        .grant(team, member.account(), Access::View)
        .await
        .unwrap();
    let mut work = admin.key_work().await.unwrap();
    work.unsealed
        .retain(|unsealed| unsealed.account != member.account());
    work.unsealed.push(Unsealed {
        key: scope,
        account: member.account(),
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&database, scope, member.account()),
        1,
        "a grantee the log names was not sealed the key"
    );

    harness.stop().await;
}
