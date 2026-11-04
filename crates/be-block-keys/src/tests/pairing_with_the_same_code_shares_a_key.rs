use super::*;

#[test]
fn pairing_with_the_same_code_shares_a_key() {
    let code = pairing_code();
    let (new_device, request) = Pairing::start(&code, b"organization");
    let (existing, reply) = Pairing::start(&code, b"organization");

    let sent = existing
        .finish(&request)
        .unwrap()
        .seal(b"the organization key");
    let received = new_device.finish(&reply).unwrap().open(&sent).unwrap();
    assert_eq!(received, b"the organization key");
}
