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
    let sealed = mine.seal_own(Own::Contact, &card.encode());
    assert_eq!(mine.open_own(Own::Contact, &sealed).unwrap(), card.encode());
    let other = RecoveryPhrase::generate().secret();
    assert_eq!(
        other.open_own(Own::Contact, &sealed),
        Err(KeyError::Corrupt)
    );
    let to_itself = mine
        .seal_for(
            [3; 16],
            &Party {
                account: [3; 16],
                public: mine.public(),
            },
            b"",
            &card.encode(),
        )
        .unwrap();
    assert_eq!(
        mine.open_own(Own::Contact, &to_itself),
        Err(KeyError::Corrupt),
        "a seal to the public key opened as one the account made for itself"
    );
    // Each kind opens only as itself: a pin is never read as a contact's record, nor one
    // organization's pin as another's.
    let pin = mine.seal_own(Own::Pin([5; 16]), b"escrow");
    assert_eq!(mine.open_own(Own::Pin([5; 16]), &pin).unwrap(), b"escrow");
    assert_eq!(
        mine.open_own(Own::Pin([6; 16]), &pin),
        Err(KeyError::Corrupt)
    );
    assert_eq!(mine.open_own(Own::Contact, &pin), Err(KeyError::Corrupt));
    assert_eq!(
        mine.open_own(Own::Pin([5; 16]), &sealed),
        Err(KeyError::Corrupt)
    );
}
