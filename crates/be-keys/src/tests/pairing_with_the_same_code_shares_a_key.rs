use super::*;

#[test]
fn pairing_with_the_same_code_shares_a_key() {
    let code = pairing_code();
    let (new_device, request) = Pairing::start(&code, b"workspace");
    let (existing, reply) = Pairing::start(&code, b"workspace");

    let sent = existing
        .finish(&request)
        .unwrap()
        .seal(b"the workspace key");
    let received = new_device.finish(&reply).unwrap().open(&sent).unwrap();
    assert_eq!(received, b"the workspace key");
}
