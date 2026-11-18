use super::*;
use crate::noise::{Initiator, Responder, hosted_prologue};

/// The Noise IK handshake of a hosted block's channel (plans/hosted_blocks.md): a joiner that
/// knows the host's static key reaches only the holder of it, which learns the joiner's static
/// key and what it sent; both split the same keys. Under another prologue (another block or
/// head), to another key, or answered by anyone but the host, it opens nothing; and the channel
/// takes each message once, in order, and nothing sealed the other way.
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
    assert!(joined.open(&sealed).is_err(), "a side opens its own message");
    assert_eq!(hosting.open(&sealed).unwrap(), b"move");
    assert!(hosting.open(&sealed).is_err(), "a message opens twice");
    let mut flipped = joined.seal(b"move");
    *flipped.last_mut().unwrap() ^= 1;
    assert!(hosting.open(&flipped).is_err(), "a changed message opens");
    let back = hosting.seal(b"seen");
    assert_eq!(joined.open(&back).unwrap(), b"seen");

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
