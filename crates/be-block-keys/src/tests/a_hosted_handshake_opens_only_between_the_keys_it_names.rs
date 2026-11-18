use super::*;
use crate::noise::{Initiator, Responder, hosted_prologue};

/// The Noise IK handshake of a hosted block's channel (plans/hosted_blocks.md): a joiner that
/// knows the host's static key reaches only the holder of it, which learns the joiner's static
/// key and what it sent; both split the same keys. Under another prologue (another block or
/// head), to another key, or answered by anyone but the host, it opens nothing; and the channel
/// takes each message once and in order, and one out of place, changed, replayed or held back,
/// breaks it.
#[test]
fn a_hosted_handshake_opens_only_between_the_keys_it_names() {
    let host = RecoverySecret::from_bytes([1; 32]);
    let joiner = RecoverySecret::from_bytes([2; 32]);
    let stranger = RecoverySecret::from_bytes([3; 32]);
    let prologue = hosted_prologue([4; 16], [5; 32]);

    let (initiator, first) = Initiator::start(&joiner, &host.public(), &prologue, b"account");
    let responder = Responder::read(&host, &prologue, &first).unwrap();
    assert_eq!(responder.joiner, joiner.public());
    assert_eq!(responder.payload, b"account");
    let (mut hosting, answer) = responder.accept(b"welcome");
    let (mut joined, said) = initiator.finish(&answer).unwrap();
    assert_eq!(said, b"welcome");
    assert_eq!(joined.handshake_hash(), hosting.handshake_hash());

    let sealed = joined.seal(b"move");
    assert_eq!(hosting.open(&sealed).unwrap(), b"move");
    let back = hosting.seal(b"seen");
    assert_eq!(joined.open(&back).unwrap(), b"seen");
    assert!(
        joined.open(&back).is_err() && joined.is_broken(),
        "a replayed message opened, or left the channel whole"
    );
    assert!(
        joined.open(&hosting.seal(b"after")).is_err(),
        "a broken channel opened another message"
    );
    let mut flipped = joined.seal(b"two");
    *flipped.last_mut().unwrap() ^= 1;
    assert!(hosting.open(&flipped).is_err(), "a changed message opened");
    assert!(hosting.is_broken());

    // A message held back breaks the channel too.
    let (initiator, first) = Initiator::start(&joiner, &host.public(), &prologue, b"account");
    let (mut hosting, answer) = Responder::read(&host, &prologue, &first)
        .unwrap()
        .accept(b"");
    let (mut joined, _) = initiator.finish(&answer).unwrap();
    let held = joined.seal(b"first");
    let later = joined.seal(b"second");
    assert!(hosting.open(&later).is_err(), "a message opened ahead of one held back");
    assert!(hosting.open(&held).is_err());

    for other in [
        hosted_prologue([6; 16], [5; 32]),
        hosted_prologue([4; 16], [6; 32]),
    ] {
        assert!(
            Responder::read(&host, &other, &first).is_err(),
            "the handshake opened under another prologue"
        );
    }
    assert!(
        Responder::read(&stranger, &prologue, &first).is_err(),
        "someone but the host read the joiner's first message"
    );

    // Someone but the host answers: it can only answer a first message made to its own key.
    let (initiator, _) = Initiator::start(&joiner, &host.public(), &prologue, b"account");
    let (_, to_stranger) = Initiator::start(&joiner, &stranger.public(), &prologue, b"account");
    let (_, forged) = Responder::read(&stranger, &prologue, &to_stranger)
        .unwrap()
        .accept(b"welcome");
    assert!(initiator.finish(&forged).is_err(), "a stranger's answer opened");
    let (initiator, _) = Initiator::start(&joiner, &host.public(), &prologue, b"account");
    assert!(
        initiator.finish(&answer).is_err(),
        "an answer to another handshake opened"
    );
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn key(text: &str) -> [u8; 32] {
    hex(text).try_into().unwrap()
}

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
