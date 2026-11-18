use super::*;

#[test]
fn an_escrow_key_signs_the_next_and_opens_what_is_sealed_to_it() {
    let first = EscrowSecret::generate();
    let second = EscrowSecret::generate();
    let kept = EscrowSecret::from_bytes(first.to_bytes());
    assert_eq!(kept.public(), first.public());
    assert_ne!(first.public(), second.public());

    // An epoch is signed by its maker's identity key, in the escrow chain's own domain.
    let maker = RecoveryPhrase::generate().secret();
    let chained = [b"next".as_slice(), &second.public().0].concat();
    let signature = maker.sign_escrow(&chained);
    assert!(maker.signing().verifies_escrow(&chained, &signature));
    assert!(
        !RecoveryPhrase::generate()
            .secret()
            .signing()
            .verifies_escrow(&chained, &signature)
    );
    assert!(
        !maker
            .signing()
            .verifies_escrow(&[b"next".as_slice(), &first.public().0].concat(), &signature)
    );
    assert!(
        !maker
            .signing()
            .verifies_authored([0; 16], &chained, &signature),
        "an escrow signature verified as something the account wrote"
    );
    assert!(
        !maker
            .signing()
            .verifies_escrow(&chained, &maker.sign_authored([0; 16], &chained)),
        "something the account wrote verified as an escrow epoch"
    );

    let sender = RecoveryPhrase::generate().secret();
    let from = Party {
        account: [1; 16],
        public: sender.public(),
    };
    let to = Party {
        account: [7; 16],
        public: first.public(),
    };
    let sealed = sender
        .seal_for(from.account, &to, b"key", &[9u8; 32])
        .unwrap();
    assert_eq!(
        first.open_from(&from, to.account, b"key", &sealed).unwrap(),
        [9u8; 32]
    );
    assert!(
        second
            .open_from(&from, to.account, b"key", &sealed)
            .is_err()
    );
    let other = Party {
        account: [2; 16],
        public: RecoveryPhrase::generate().secret().public(),
    };
    assert!(
        first
            .open_from(&other, to.account, b"key", &sealed)
            .is_err(),
        "a wrap opens only as from the sender who sealed it"
    );
}
