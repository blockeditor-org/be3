use super::*;

use be_block_graph::{ANYONE_INSIDE, Access, PREFIXES_TYPE};
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

/// A kind of settings is offered to a viewer's contacts only when its own log opens it to
/// anyone inside (signed access 3c): a server that turns an ordinary block into a kind open
/// inside, and lists a contact as due its key, gets nothing sealed to them; a kind its author
/// opened by a signed opening is sealed to the same contact.
#[tokio::test]
async fn a_block_the_server_says_is_open_inside_is_sealed_to_no_contact() {
    let mut harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    assert!(admin.verified_identity(member.account()).is_some());

    // A kind its author opens to anyone inside, in a workspace the member views.
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
    let opened = admin.keys().scope_key(kind).unwrap();

    // A note with a scope of its own, in a workspace the member does not view.
    let private = workspace(&admin, "Private").await;
    let note = admin
        .create::<be_type_text::TextContent>(BlockParent::Block(private))
        .await
        .unwrap();
    admin.offer(note, Access::View).await.unwrap();
    let secret = admin.keys().scope_key(note).unwrap();

    // The server makes the note a kind open inside.
    let database = harness.database();
    let organization = admin.organization().to_string();
    database
        .execute(
            "UPDATE blocks SET content_type = ?1 WHERE organization_id = ?2 AND id = ?3",
            [
                PREFIXES_TYPE.to_string(),
                organization.clone(),
                note.to_string(),
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT OR REPLACE INTO block_access (organization_id, block_id, account_id, access)
             VALUES (?1, ?2, ?3, 'view')",
            [organization, note.to_string(), ANYONE_INSIDE.to_string()],
        )
        .unwrap();
    harness.server.restart().await;
    harness.url = format!("ws://{}", harness.server.address);
    harness.reconnect(&admin, admin.token()).await;
    assert!(
        admin.summary(note).await.unwrap().opens_inside,
        "the server says the note is open inside"
    );

    let mut work = admin.key_work().await.unwrap();
    for key in [secret, opened] {
        work.unsealed.retain(|unsealed| unsealed.key != key);
        work.unsealed.push(Unsealed {
            key,
            account: member.account(),
        });
    }
    admin.tend_with(work).await.unwrap();
    assert_eq!(
        sealed(&database, secret, member.account()),
        0,
        "a block no log opens was sealed to a contact the server lists"
    );
    assert_eq!(
        sealed(&database, opened, member.account()),
        1,
        "a kind its log opens was not offered to a contact viewing inside"
    );

    harness.stop().await;
}
