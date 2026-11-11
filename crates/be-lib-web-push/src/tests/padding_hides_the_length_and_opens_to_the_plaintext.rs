use super::*;

#[test]
fn padding_hides_the_length_and_opens_to_the_plaintext() {
    let random = SystemRandom::new();
    let mut sealed = Vec::new();
    for plain in [&b"hi"[..], &[7; 300][..]] {
        let device =
            agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &random).unwrap();
        let public = device.compute_public_key().unwrap().as_ref().to_vec();
        let auth = [9; 16];
        let body = encrypt(&public, &auth, plain, 512).unwrap();
        let sender = agreement::UnparsedPublicKey::new(
            &agreement::ECDH_P256,
            sender_key(&body).unwrap().to_vec(),
        );
        let opened = agreement::agree_ephemeral(device, &sender, |secret| {
            open(secret, &public, &auth, &body)
        })
        .unwrap();
        assert_eq!(opened.as_deref(), Some(plain));
        sealed.push(body.len());
    }
    assert_eq!(sealed[0], sealed[1], "both pushes are padded to one length");
    assert_eq!(
        encrypt(&[4; 65], &[0; 15], b"x", 0),
        Err(Error::Keys),
        "a secret of the wrong length is refused"
    );
    assert_eq!(
        encrypt(&[4; 65], &[0; 16], &[0; MAX_PLAIN_BYTES + 1], 0),
        Err(Error::TooLong)
    );
}
