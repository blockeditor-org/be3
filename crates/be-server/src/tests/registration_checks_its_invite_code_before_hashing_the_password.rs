use super::*;
use crate::store::passwords::PASSWORD_CHECKS_PER_ADDRESS;

#[tokio::test]
async fn registration_checks_its_invite_code_before_hashing_the_password() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let held: Vec<_> = (0..PASSWORD_CHECKS_PER_ADDRESS)
        .map(|_| store.passwords.admit("192.0.2.1").unwrap())
        .collect();
    let wrong = store
        .registration_link(
            "a@example.com",
            "A",
            "a good password",
            "wrong",
            "",
            "192.0.2.1",
        )
        .await;
    assert!(
        matches!(&wrong, Err(ServerError::Refused(_, message)) if message.starts_with("Invite code")),
        "{:?}",
        wrong.err()
    );
    let invite = harness.invite();
    let busy = store
        .registration_link(
            "a@example.com",
            "A",
            "a good password",
            &invite,
            "",
            "192.0.2.1",
        )
        .await;
    assert!(
        matches!(&busy, Err(ServerError::Refused(_, message)) if message.contains("busy")),
        "{:?}",
        busy.err()
    );
    drop(held);
    store
        .registration_link(
            "a@example.com",
            "A",
            "a good password",
            &invite,
            "",
            "192.0.2.1",
        )
        .await
        .unwrap();
}
