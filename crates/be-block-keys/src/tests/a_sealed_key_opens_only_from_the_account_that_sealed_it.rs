use super::*;

fn party(account: u8, secret: &RecoverySecret) -> Party {
    Party {
        account: [account; 16],
        public: secret.public(),
    }
}

#[test]
fn a_sealed_key_opens_only_from_the_account_that_sealed_it() {
    let sender = RecoveryPhrase::generate().secret();
    let recipient = RecoveryPhrase::generate().secret();
    let server = RecoveryPhrase::generate().secret();
    let key = [7u8; 32];
    let to = party(2, &recipient);

    let sealed = sender.seal_for([1; 16], &to, b"key", &key);
    assert_ne!(sealed, sender.seal_for([1; 16], &to, b"key", &key));
    assert_eq!(
        recipient
            .open_from(&party(1, &sender), [2; 16], b"key", &sealed)
            .unwrap(),
        key
    );
    let kept = RecoverySecret::from_bytes(recipient.to_bytes());
    assert_eq!(
        kept.open_from(&party(1, &sender), [2; 16], b"key", &sealed)
            .unwrap(),
        key
    );

    for (from, to_account, context) in [
        (party(1, &server), [2; 16], b"key".as_slice()),
        (party(3, &sender), [2; 16], b"key"),
        (party(1, &sender), [4; 16], b"key"),
        (party(1, &sender), [2; 16], b"other"),
    ] {
        assert_eq!(
            recipient.open_from(&from, to_account, context, &sealed),
            Err(KeyError::Sender)
        );
    }
    assert_eq!(
        server.open_from(&party(1, &sender), [2; 16], b"key", &sealed),
        Err(KeyError::Sender)
    );

    let forged = recipient.public().seal_anonymous(&key);
    assert_eq!(
        recipient.open_from(&party(1, &sender), [2; 16], b"key", &forged),
        Err(KeyError::Sender),
        "a key anyone could seal to the public key opened as if the sender had sealed it"
    );
    let impostor = server.seal_for([1; 16], &to, b"key", &key);
    assert_eq!(
        recipient.open_from(&party(1, &sender), [2; 16], b"key", &impostor),
        Err(KeyError::Sender)
    );

    let mut flipped = sealed.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 1;
    assert_eq!(
        recipient.open_from(&party(1, &sender), [2; 16], b"key", &flipped),
        Err(KeyError::Sender)
    );

    let mine = party(1, &sender);
    let own = sender.seal_for([1; 16], &mine, b"key", &key);
    assert_eq!(sender.open_from(&mine, [1; 16], b"key", &own).unwrap(), key);
}
