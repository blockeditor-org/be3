use super::*;

/// The handshake, run on Noise's own ChaChaPoly, makes the bytes of the published
/// `Noise_IK_25519_ChaChaPoly_SHA256` vector (cacophony's, as snow's tests carry it): both
/// messages and the handshake hash, so a mistake both sides would make alike shows.
#[test]
fn the_hosted_handshake_makes_noises_published_ik_vector() {
    use crate::noise::Cipher;
    use x25519_dalek::StaticSecret;

    let prologue = hex("4a6f686e2047616c74");
    let initiator = RecoverySecret::from_bytes(key(
        "e61ef9919cde45dd5f82166404bd08e38bceb5dfdfded0a34c8df7ed542214d1",
    ));
    let responder = RecoverySecret::from_bytes(key(
        "4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893",
    ));
    assert_eq!(
        responder.public().0,
        key("31e0303fd6418d2f8c0e78b91f22e8caed0fbe48656dcf4767e4834f701b8f62"),
        "the vector's remote static key is the responder's"
    );
    let (started, first) = Initiator::start_with(
        Cipher::ChaChaPoly,
        StaticSecret::from(key(
            "893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a",
        )),
        &initiator,
        &responder.public(),
        &prologue,
        &hex("4c756477696720766f6e204d69736573"),
    );
    assert_eq!(
        first,
        hex(
            "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c7944718da798efbcd915\
             28520204f904b9bd6c7413dccdc214d951e15253e39987f18146e8cd0873654207148333479d4d16\
             c289f0294b29960a72f48e0b7bba2e89083169825e59642148d492020664ccf7"
        )
    );
    let read = Responder::read_with(Cipher::ChaChaPoly, &responder, &prologue, &first).unwrap();
    assert_eq!(read.joiner, initiator.public());
    assert_eq!(read.payload, hex("4c756477696720766f6e204d69736573"));
    let (hosting, answer) = read.accept_with(
        StaticSecret::from(key(
            "bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b",
        )),
        &hex("4d757272617920526f746862617264"),
    );
    assert_eq!(
        answer,
        hex(
            "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f1448088435361e70b2ed446e6\
             c9ec387d1d6b3b840f194e373979d241b203c4acafccf5"
        )
    );
    let (joined, said) = started.finish(&answer).unwrap();
    assert_eq!(said, hex("4d757272617920526f746862617264"));
    let hash = key("0b0f68fb0c27e03ce9b97565995ed4838cc0581b762ef72b062f6a546419fad7");
    assert_eq!(joined.handshake_hash(), hash);
    assert_eq!(hosting.handshake_hash(), hash);
}
