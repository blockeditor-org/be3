use super::*;

#[test]
fn an_escrow_key_signs_the_next_and_opens_what_is_sealed_to_it() {
    let first = EscrowSecret::generate();
    let second = EscrowSecret::generate();
    let kept = EscrowSecret::from_bytes(first.to_bytes());
    assert_eq!(kept.public(), first.public());
    assert_eq!(kept.signing(), first.signing());
    assert_ne!(first.signing(), second.signing());
    assert_ne!(first.signing(), first.public().0);

    let message = [b"next".as_slice(), &second.public().0, &second.signing()].concat();
    let signature = first.sign(&message);
    assert!(verify_signature(&first.signing(), &message, &signature));
    assert!(!verify_signature(&second.signing(), &message, &signature));
    assert!(!verify_signature(
        &first.signing(),
        &[b"next".as_slice(), &first.public().0].concat(),
        &signature
    ));
    assert!(!verify_signature(&first.signing(), &message, &[]));
    assert!(!verify_signature(
        &first.signing(),
        &message,
        &second.sign(&message)
    ));

    let sender = RecoveryPhrase::generate().secret();
    let from = Party {
        account: [1; 16],
        public: sender.public(),
    };
    let to = Party {
        account: [7; 16],
        public: first.public(),
    };
    let sealed = sender.seal_for(from.account, &to, b"key", &[9u8; 32]);
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
    assert!(
        first
            .open_from(
                &from,
                to.account,
                b"key",
                &to.public.seal_anonymous(&[9u8; 32])
            )
            .is_err(),
        "an anonymous wrap opens as from no one"
    );

    let organization = [5; 16];
    let pinned = RecoveryPhrase::generate().secret();
    let pin = pinned.seal_pin(organization, b"escrow");
    assert_eq!(pinned.open_pin(organization, &pin).unwrap(), b"escrow");
    assert!(pinned.open_pin([6; 16], &pin).is_err());
}
