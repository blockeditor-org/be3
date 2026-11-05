use super::*;
use crate::store::portal::MAX_MAIL_LINKS_PER_ADDRESS;

#[tokio::test]
async fn mail_links_are_bounded_per_address_and_unknown_resets_cost_the_same() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    for index in 0..MAX_MAIL_LINKS_PER_ADDRESS {
        store
            .register(
                &format!("user{index}@example.com"),
                "User",
                "a good password",
            )
            .await
            .unwrap();
    }
    for index in 0..MAX_MAIL_LINKS_PER_ADDRESS {
        let link = store
            .password_reset_link(&format!("user{index}@example.com"), "", "192.0.2.1")
            .await
            .unwrap();
        assert!(link.is_some());
    }
    assert!(
        store
            .password_reset_link("user0@example.com", "", "192.0.2.1")
            .await
            .is_err()
    );
    // Another address isn't held back by the first one's links.
    assert!(
        store
            .password_reset_link("user0@example.com", "", "198.51.100.7")
            .await
            .unwrap()
            .is_some()
    );
    // A reset for an email with no account sends nothing but counts the same.
    for index in 0..MAX_MAIL_LINKS_PER_ADDRESS {
        let link = store
            .password_reset_link(&format!("nobody{index}@example.com"), "", "203.0.113.5")
            .await
            .unwrap();
        assert!(link.is_none());
    }
    assert!(
        store
            .password_reset_link("user1@example.com", "", "203.0.113.5")
            .await
            .is_err()
    );
}
