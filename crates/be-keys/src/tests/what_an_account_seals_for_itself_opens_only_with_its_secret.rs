use super::*;

#[test]
fn what_an_account_seals_for_itself_opens_only_with_its_secret() {
    let mine = RecoveryPhrase::generate().secret();
    let card = ContactCard {
        account: [3; 16],
        public: RecoveryPhrase::generate().secret().public(),
        display_name: "Grace".into(),
        email: "grace@example.com".into(),
    };
    let sealed = card.seal_own(&mine);
    assert_eq!(ContactCard::open_own(&mine, &sealed).unwrap(), card);
    let other = RecoveryPhrase::generate().secret();
    assert_eq!(
        ContactCard::open_own(&other, &sealed),
        Err(KeyError::Corrupt)
    );
    assert_eq!(
        ContactCard::open_own(&mine, &mine.public().seal(&card.encode())),
        Err(KeyError::Corrupt),
        "anyone who knows the public key could seal a contact the owner would trust"
    );
}
