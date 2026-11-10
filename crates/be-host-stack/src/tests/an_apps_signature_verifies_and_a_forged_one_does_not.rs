use super::*;
use crate::authorship::{self, Signer};

fn signed(block: Uuid, bytes: &[u8]) -> Vec<u8> {
    let (sender, receiver) = std::sync::mpsc::channel();
    authorship::sign(block, bytes.to_vec(), move |signed| {
        let _ = sender.send(signed);
    });
    receiver
        .recv()
        .expect("the worker answered")
        .expect("the worker signed")
}

fn verified(account: Uuid, block: Uuid, bytes: &[u8], signature: Vec<u8>) -> Signer {
    let (sender, receiver) = std::sync::mpsc::channel();
    authorship::verify(account, block, bytes.to_vec(), signature, move |found| {
        let _ = sender.send(found);
    });
    receiver
        .recv()
        .expect("the worker answered")
        .expect("the worker verified")
}

#[test]
fn an_apps_signature_verifies_and_a_forged_one_does_not() {
    let harness = Harness::start();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();

    let mine = signed(block, b"hello");
    assert_eq!(
        verified(harness.account, block, b"hello", mine.clone()),
        Signer::Verified
    );
    assert_eq!(
        verified(harness.account, block, b"hullo", mine.clone()),
        Signer::Bad
    );

    let runtime = runtime();
    let peer = runtime.block_on(guest.peer(&harness.url, personal));
    let theirs = peer.sign_authored(block, b"hello");
    assert_eq!(
        verified(guest.session.account, block, b"hello", theirs.clone()),
        Signer::Unverified,
        "a member who is not a contact is known by the key the server names"
    );
    assert_eq!(
        verified(harness.account, block, b"hello", theirs),
        Signer::Bad,
        "the guest's signature does not pass for this account's"
    );
    let mut flipped = mine;
    flipped[10] ^= 1;
    assert_eq!(
        verified(harness.account, block, b"hello", flipped),
        Signer::Bad
    );
}
