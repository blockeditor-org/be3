use super::*;

#[test]
fn a_contact_card_opens_only_with_its_code_in_its_role() {
    let code = InviteCode::generate();
    let card = ContactCard {
        account: [7; 16],
        public: RecoveryPhrase::generate().secret().public(),
        display_name: "Ada".into(),
        email: "ada@example.com".into(),
    };
    let sealed = card.seal(&code, CardRole::Owner);
    assert_eq!(
        ContactCard::open(&code, CardRole::Owner, &sealed).unwrap(),
        card
    );
    assert_eq!(
        ContactCard::open(&code, CardRole::Accepter, &sealed),
        Err(KeyError::Card),
        "the owner's card opened as the accepter's"
    );
    assert_eq!(
        ContactCard::open(&InviteCode::generate(), CardRole::Owner, &sealed),
        Err(KeyError::Card),
        "another code opened the card"
    );
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert_eq!(
        ContactCard::open(&code, CardRole::Owner, &tampered),
        Err(KeyError::Card),
        "a changed card still opened"
    );
    let forged = ContactCard {
        public: RecoveryPhrase::generate().secret().public(),
        ..card.clone()
    };
    let swapped = forged.seal(&InviteCode::generate(), CardRole::Owner);
    assert_eq!(
        ContactCard::open(&code, CardRole::Owner, &swapped),
        Err(KeyError::Card),
        "a card made without the code opened"
    );

    let key = [9u8; 32];
    let grant = code.seal_grant([1; 16], &key);
    assert_eq!(code.open_grant([1; 16], &grant).unwrap(), key);
    assert!(
        code.open_grant([2; 16], &grant).is_err(),
        "a grant's key opened under another key's id"
    );
    assert!(InviteCode::from_bytes(&code.to_bytes()).unwrap() == code);
}
