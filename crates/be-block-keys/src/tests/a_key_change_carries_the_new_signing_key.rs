use super::*;

#[test]
fn a_key_change_carries_the_new_signing_key() {
    let old = RecoveryPhrase::generate().secret();
    let fresh = RecoveryPhrase::generate().secret();
    let contact = RecoveryPhrase::generate().secret();
    let (account, theirs) = ([1; 16], [2; 16]);
    let to = Party {
        account: theirs,
        public: contact.public(),
    };
    let sealed = old.seal_key_change(account, &to, fresh.identity());
    let from = Party {
        account,
        public: old.public(),
    };
    let changed = contact.open_key_change(&from, theirs, &sealed).unwrap();
    assert_eq!(changed, fresh.identity());
    assert!(changed.signing.verifies_authored(
        [3; 16],
        b"after",
        &fresh.sign_authored([3; 16], b"after")
    ));

    let forger = RecoveryPhrase::generate().secret();
    let forged = forger.seal_key_change(account, &to, forger.identity());
    assert_eq!(
        contact.open_key_change(&from, theirs, &forged),
        Err(KeyError::Sender),
        "only the key the contact verified can name the next one"
    );
}
