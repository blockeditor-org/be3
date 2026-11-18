use super::*;

/// A device that holds the account's identity secret, told by the server that the account has
/// no recovery key, makes no new phrase (which would replace the secret it keeps) and says what
/// happened; one that holds none makes the account's first phrase (guides/blocks/keys.md, "Identity, new phrases and devices").
#[test]
fn a_server_that_says_the_account_has_no_key_never_has_a_device_replace_its_own() {
    let keys = || crate::accounts::Keys {
        account: Uuid::new_v4(),
        recovery: None,
        sealed: Vec::new(),
        logs: Vec::new(),
    };
    let held = be_block_keys::RecoveryPhrase::generate().secret().to_bytes();

    let mut holding = crate::keys::KeyState::default();
    holding.loaded(keys(), Some(held));
    assert!(
        !holding.needs_recovery(),
        "a device holding the account's key was asked to save a new phrase"
    );
    assert_eq!(holding.lost(), Some(crate::keys::SERVER_LOST_KEY));
    assert!(holding.recovery_view().words.is_empty());

    let mut fresh = crate::keys::KeyState::default();
    fresh.loaded(keys(), None);
    assert!(fresh.needs_recovery());
    assert_eq!(fresh.lost(), None);
    assert_eq!(fresh.recovery_view().words.len(), 12);
}
