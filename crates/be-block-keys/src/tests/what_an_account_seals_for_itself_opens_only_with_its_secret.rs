use super::*;

#[test]
fn what_an_account_seals_for_itself_opens_only_with_its_secret() {
    let mine = RecoveryPhrase::generate().secret();
    let card = ContactCard {
        account: [3; 16],
        public: RecoveryPhrase::generate().secret().public(),
        signing: RecoveryPhrase::generate().secret().signing(),
        display_name: "Grace".into(),
        email: "grace@example.com".into(),
    };
    let sealed = mine.seal_own(&card.encode());
    assert_eq!(mine.open_own(&sealed).unwrap(), card.encode());
    let other = RecoveryPhrase::generate().secret();
    assert_eq!(other.open_own(&sealed), Err(KeyError::Corrupt));
    assert_eq!(
        mine.open_own(&mine.public().seal_anonymous(&card.encode())),
        Err(KeyError::Corrupt),
        "anyone who knows the public key could seal a contact the owner would trust"
    );
}
