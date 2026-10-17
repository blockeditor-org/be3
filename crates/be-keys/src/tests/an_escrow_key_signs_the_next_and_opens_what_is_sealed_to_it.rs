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

    let sealed = first.public().seal_anonymous(&[9u8; 32]);
    assert_eq!(first.open(&sealed).unwrap(), [9u8; 32]);
    assert_eq!(second.open(&sealed), Err(KeyError::Corrupt));

    let organization = [5; 16];
    let pinned = RecoveryPhrase::generate().secret();
    let pin = pinned.seal_pin(organization, b"escrow");
    assert_eq!(pinned.open_pin(organization, &pin).unwrap(), b"escrow");
    assert!(pinned.open_pin([6; 16], &pin).is_err());
}
