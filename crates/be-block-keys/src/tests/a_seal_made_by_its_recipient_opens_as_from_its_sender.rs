use super::*;

/// A recipient seals to itself what opens as from a sender (`seal_as_from`), as the sender's
/// own seal does, and from no other sender.
#[test]
fn a_seal_made_by_its_recipient_opens_as_from_its_sender() {
    let sender = RecoveryPhrase::generate().secret();
    let recipient = RecoveryPhrase::generate().secret();
    let from = Party {
        account: [1; 16],
        public: sender.public(),
    };
    let sealed = recipient
        .seal_as_from(&from, [2; 16], b"key", b"plain")
        .unwrap();
    assert_eq!(
        recipient.open_from(&from, [2; 16], b"key", &sealed).unwrap(),
        b"plain"
    );
    let other = Party {
        account: [1; 16],
        public: RecoveryPhrase::generate().secret().public(),
    };
    assert!(recipient.open_from(&other, [2; 16], b"key", &sealed).is_err());
}
