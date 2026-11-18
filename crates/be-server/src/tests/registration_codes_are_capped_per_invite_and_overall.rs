use super::*;
use crate::store::codes::{
    self, CodePurpose, CodeRequest, MAX_REGISTRATION_CODES, REGISTRATION_CODES_PER_INVITE,
};

async fn ask(
    store: &ServerStore,
    invite: &str,
    email: &str,
    address: &str,
) -> Result<Option<codes::MailedCode>, ServerError> {
    store
        .request_code(CodeRequest {
            purpose: CodePurpose::Register,
            email,
            account: None,
            display_name: "New",
            invite,
            device_code: "",
            attempt: &crate::new_secret(),
            address,
        })
        .await
}

#[tokio::test]
async fn registration_codes_are_capped_per_invite_and_overall() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    // One invite code can't hold more than its share of waiting registrations, from however
    // many addresses.
    let invite = harness.invite();
    for index in 0..REGISTRATION_CODES_PER_INVITE {
        ask(
            &store,
            &invite,
            &format!("wait{index}@example.com"),
            &format!("10.{index}.0.1"),
        )
        .await
        .unwrap()
        .unwrap();
    }
    let refused = ask(&store, &invite, "late@example.com", "192.0.2.1").await;
    assert!(
        matches!(&refused, Err(ServerError::Refused(_, message)) if message.starts_with("Invite code")),
        "{:?}",
        refused.err().map(|error| error.to_string())
    );
    // Nor can every invite code together hold more than the server's cap.
    let database = metadata_database(&harness.directory);
    for index in 0..MAX_REGISTRATION_CODES {
        database
            .execute(
                "INSERT INTO sign_in_codes (attempt_hash, code_hash, purpose, email, expires, created, address, network)
                 VALUES (?1, '', 'register', ?2, ?3, unixepoch(), ?4, ?4)",
                rusqlite::params![
                    format!("filler-{index}"),
                    format!("filler{index}@example.com"),
                    i64::MAX,
                    format!("filler-{index}")
                ],
            )
            .unwrap();
    }
    let refused = ask(
        &store,
        &harness.invite(),
        "other@example.com",
        "203.0.113.2",
    )
    .await;
    assert!(
        matches!(&refused, Err(ServerError::Refused(_, message)) if message.starts_with("Too many")),
        "{:?}",
        refused.err().map(|error| error.to_string())
    );
}
