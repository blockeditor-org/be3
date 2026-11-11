use super::*;

#[test]
fn a_contact_verifies_what_another_signed_with_the_key_on_its_card() {
    let ada = RecoveryPhrase::generate().secret();
    let code = InviteCode::generate();
    let card = ContactCard {
        account: [7; 16],
        public: ada.public(),
        signing: ada.signing(),
        display_name: "Ada".into(),
        email: "ada@example.com".into(),
    };
    let opened = ContactCard::open(&code, CardRole::Owner, &card.seal(&code, CardRole::Owner))
        .unwrap()
        .signing;
    let block = [1; 16];
    let signature = ada.sign_authored(block, b"hello");
    assert!(opened.verifies_authored(block, b"hello", &signature));
    assert!(
        !opened.verifies_authored(block, b"hullo", &signature),
        "the bytes changed since they were signed"
    );
    assert!(
        !opened.verifies_authored([2; 16], b"hello", &signature),
        "a signature made for one block never verifies in another"
    );
    let mallory = RecoveryPhrase::generate().secret();
    assert!(
        !opened.verifies_authored(block, b"hello", &mallory.sign_authored(block, b"hello")),
        "someone else's signature is not Ada's"
    );
    let mut flipped = signature.clone();
    flipped[0] ^= 1;
    assert!(!opened.verifies_authored(block, b"hello", &flipped));
    assert!(!opened.verifies_authored(block, b"hello", &signature[..63]));
    assert!(!opened.verifies_authored(block, b"hello", &[]));
    assert_ne!(
        ada.signing().0,
        ada.public().0,
        "the signing key is derived apart from the identity key"
    );
    assert_eq!(
        RecoverySecret::from_bytes(ada.to_bytes()).signing(),
        ada.signing(),
        "the same secret always derives the same signing key"
    );
}
