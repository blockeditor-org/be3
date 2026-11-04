use super::*;
use crate::store::portal::{MAX_MAIL_LINKS, MAX_MAIL_LINKS_PER_INVITE};

fn fill(directory: &std::path::Path, purpose: &str, rows: i64) {
    let database = rusqlite::Connection::open(directory.join("metadata.sqlite")).unwrap();
    for index in 0..rows {
        database
            .execute(
                "INSERT INTO account_mail (token_hash, purpose, email, expires, address) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    format!("{purpose}-{index}"),
                    purpose,
                    format!("filler{index}@example.com"),
                    i64::MAX,
                    format!("10.9.{}.{}", index / 200, index % 200)
                ],
            )
            .unwrap();
    }
}

#[tokio::test]
async fn anonymous_mail_requests_never_fill_the_global_cap() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    store
        .register("owner@example.com", "Owner", "a good password")
        .await
        .unwrap();
    // Resets, sent or not, from as many addresses as an attacker likes,
    // leave registration and other resets alone.
    fill(&harness.directory, "unsent", MAX_MAIL_LINKS);
    fill(&harness.directory, "reset", MAX_MAIL_LINKS);
    let invite = harness.invite();
    store
        .registration_link(
            "new@example.com",
            "New",
            "a good password",
            &invite,
            "",
            "192.0.2.1",
        )
        .await
        .unwrap();
    assert!(
        store
            .password_reset_link("owner@example.com", "", "192.0.2.2")
            .await
            .unwrap()
            .is_some()
    );
    // An unknown email still answers the same once unsent rows are full.
    assert!(
        store
            .password_reset_link("nobody@example.com", "", "192.0.2.3")
            .await
            .unwrap()
            .is_none()
    );

    // Registrations waiting on one invite code are bounded, so one code
    // can't fill the cap either.
    for index in 1..MAX_MAIL_LINKS_PER_INVITE {
        store
            .registration_link(
                &format!("wait{index}@example.com"),
                "Waiting",
                "a good password",
                &invite,
                "",
                &format!("198.51.100.{index}"),
            )
            .await
            .unwrap();
    }
    let refused = store
        .registration_link(
            "late@example.com",
            "Late",
            "a good password",
            &invite,
            "",
            "203.0.113.1",
        )
        .await;
    assert!(
        matches!(&refused, Err(ServerError::Refused(_, message)) if message.starts_with("Invite code")),
        "{:?}",
        refused.err()
    );

    // The cap itself still holds for registrations.
    fill(&harness.directory, "register", MAX_MAIL_LINKS);
    assert!(
        store
            .registration_link(
                "other@example.com",
                "Other",
                "a good password",
                &harness.invite(),
                "",
                "203.0.113.2"
            )
            .await
            .is_err()
    );
}
