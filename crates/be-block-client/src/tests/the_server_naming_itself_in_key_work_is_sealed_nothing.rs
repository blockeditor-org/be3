use super::*;

use be_block_protocol::Unsealed;

/// What a device seals to the server's account: counted in the server's own rows.
fn sealed_to(database: &rusqlite::Connection, account: Uuid) -> i64 {
    database
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys WHERE wrapped_by = ?1",
            [format!("account:{account}")],
            |row| row.get(0),
        )
        .unwrap()
}

/// A device seals a key only to itself or a verified contact: the server naming its own
/// account in key work, as one a scope key is not sealed to yet or as an administrator due an
/// escrow, is sealed nothing. Only `share_with_server` seals to it, and only what it names.
#[tokio::test]
async fn the_server_naming_itself_in_key_work_is_sealed_nothing() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let private = workspace(&admin, "Private").await;
    let scope = admin.keys().scope_key(private).unwrap();
    let database = harness.database();
    let server: Uuid = database
        .query_row(
            "SELECT account_id FROM server_account WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .parse()
        .unwrap();
    let before = sealed_to(&database, server);

    // The server lists itself as an account the workspace's scope key is not sealed to yet.
    let mut work = admin.key_work().await.unwrap();
    work.unsealed.push(Unsealed {
        key: scope,
        account: server,
    });
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed_to(&database, server),
        before,
        "no scope key is sealed to the server"
    );

    // And as an administrator, with the escrow due a new epoch.
    database
        .execute(
            "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'administrator')",
            [admin.organization().to_string(), server.to_string()],
        )
        .unwrap();
    let mut work = admin.key_work().await.unwrap();
    work.escrow_rotation = true;
    let escrow = admin.keys().escrow().expect("the creator holds the escrow");
    admin.tend_with(work).await.unwrap();
    assert_ne!(
        admin.keys().escrow(),
        Some(escrow),
        "the escrow moved on to a new epoch"
    );
    assert_eq!(
        sealed_to(&database, server),
        before,
        "nor is the new escrow sealed to it"
    );
}
