use super::*;

use be_block_keys::{Party, RecoveryPhrase};

/// A key a contact sealed to a key this account replaced opens with the retired secret only
/// while the contact may still hold that key (`contacts::behind`): once its record shows it
/// holds the current key, the same seal, which whoever holds the retired secret could make in
/// its name, is refused.
#[test]
fn a_retired_secret_opens_only_what_a_contact_that_may_hold_its_key_seals() {
    let account = Uuid::now_v7();
    let old = RecoveryPhrase::generate().secret();
    let current = RecoveryPhrase::generate().secret();
    let contact = RecoveryPhrase::generate().secret();
    let from = Party {
        account: Uuid::now_v7().into_bytes(),
        public: contact.public(),
    };
    let to_old = Party {
        account: account.into_bytes(),
        public: old.public(),
    };
    let sealed = contact
        .seal_for(from.account, &to_old, b"key", b"plain")
        .unwrap();
    let retired = [old];
    let open = |secret: &be_block_keys::RecoverySecret| {
        secret.open_from(&from, account.into_bytes(), b"key", &sealed)
    };

    let stale = crate::contacts::behind(Some(retired[0].public()), &current);
    let (plain, secret) = crate::contacts::open_by_any(&current, &retired, stale, open).unwrap();
    assert_eq!(plain, b"plain");
    assert_eq!(
        secret.public(),
        retired[0].public(),
        "not opened with the retired secret"
    );

    let followed = crate::contacts::behind(Some(current.public()), &current);
    assert!(
        !followed,
        "a contact that holds the current key counts as behind"
    );
    assert!(
        crate::contacts::open_by_any(&current, &retired, followed, open).is_err(),
        "a retired secret opened what a contact that holds the current key sealed"
    );
}
