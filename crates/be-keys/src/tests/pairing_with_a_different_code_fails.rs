use super::*;

#[test]
fn pairing_with_a_different_code_fails() {
    let (new_device, request) = Pairing::start(&pairing_code(), b"organization");
    let (existing, reply) = Pairing::start(&pairing_code(), b"organization");
    let sent = existing
        .finish(&request)
        .unwrap()
        .seal(b"the organization key");
    assert_eq!(
        new_device.finish(&reply).unwrap().open(&sent),
        Err(KeyError::Pairing)
    );

    let code = pairing_code();
    let (new_device, request) = Pairing::start(&code, b"organization");
    let (existing, reply) = Pairing::start(&code, b"another organization");
    let sent = existing
        .finish(&request)
        .unwrap()
        .seal(b"the organization key");
    assert_eq!(
        new_device.finish(&reply).unwrap().open(&sent),
        Err(KeyError::Pairing),
        "pairing for one organization unlocked another"
    );
}
