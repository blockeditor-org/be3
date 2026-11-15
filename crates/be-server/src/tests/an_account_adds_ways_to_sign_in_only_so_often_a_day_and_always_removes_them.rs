use super::*;
use crate::store::account_notices::{IDENTITY_CHANGES_PER_DAY, MAX_ACCOUNT_NOTICES};

#[tokio::test]
async fn an_account_adds_ways_to_sign_in_only_so_often_a_day_and_always_removes_them() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let account = ServerStore::open(&harness.directory)
        .unwrap()
        .browser_profile(&ada.cookie)
        .await
        .unwrap()
        .account;
    ada.add_passkey(&harness).await;
    // A day's additions made, and as many fresh notices as the account keeps: the next addition
    // is refused, so none of those notices can be pushed out by adding more.
    let database = metadata_database(&harness.directory);
    for _ in 1..IDENTITY_CHANGES_PER_DAY {
        database
            .execute(
                "INSERT INTO identity_changes (account_id, kind, label, created)
                 VALUES (?1, 'added', 'Passkey', unixepoch())",
                [account.to_string()],
            )
            .unwrap();
    }
    for _ in 1..MAX_ACCOUNT_NOTICES {
        database
            .execute(
                "INSERT INTO account_notices (account_id, text, created)
                 VALUES (?1, 'A passkey can now sign in.', unixepoch())",
                [account.to_string()],
            )
            .unwrap();
    }
    let origin = ada.origin();
    let (status, response) = ada
        .passkey(&harness, "/auth/settings", |options| {
            crate::fake_authenticator::FakeAuthenticator::create(options, &origin, None).1
        })
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("were added today"), "{response}");
    // Past the bound, a notice in its first week is never let go of.
    let held: i64 = database
        .query_row(
            "SELECT count(*) FROM account_notices WHERE account_id = ?1",
            [account.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(held, MAX_ACCOUNT_NOTICES);
    // A removal is never capped: the owner always removes what an intruder added, and it is
    // still told and listed.
    let id = page_of(&ada.get(&harness, "/auth/settings").await)
        .identities
        .into_iter()
        .find(|identity| identity.kind == "passkey")
        .unwrap()
        .id;
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &id)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let held: i64 = database
        .query_row(
            "SELECT count(*) FROM account_notices WHERE account_id = ?1",
            [account.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(held, MAX_ACCOUNT_NOTICES + 1, "none in its first week went");
    let changes = page_of(&ada.get(&harness, "/auth/settings").await).identity_changes;
    assert!(
        changes[0].text.starts_with("Removed Passkey added "),
        "{changes:?}"
    );
    // A day on, it adds again.
    database
        .execute("UPDATE identity_changes SET created = created - 86400", [])
        .unwrap();
    ada.add_passkey(&harness).await;
}
