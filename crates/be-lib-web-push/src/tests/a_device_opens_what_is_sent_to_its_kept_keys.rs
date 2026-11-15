use super::*;
use base64::Engine;

/// A device's keys open RFC 8291's example message with its private key, and keys it makes
/// open what is encrypted to them after being kept and read back, and nothing sent to others.
#[test]
fn a_device_opens_what_is_sent_to_its_kept_keys() {
    let decode = |text: &str| {
        let text: String = text.split_whitespace().collect();
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(text)
            .unwrap()
    };
    let mut example = decode("q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94");
    example.extend(decode("BTBZMqHH6r4Tts7J_aSIgg"));
    let example = DeviceKeys::from_bytes(&example).unwrap();
    assert_eq!(
        example.public(),
        decode(
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcx
             aOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4"
        )
    );
    let message = decode(
        "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27ml
         mlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPT
         pK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN",
    );
    assert_eq!(
        example.open(&message).as_deref(),
        Some(&b"When I grow up, I want to be a watermelon"[..])
    );

    let made = DeviceKeys::generate();
    let kept = DeviceKeys::from_bytes(&made.to_bytes()).unwrap();
    assert_eq!(kept, made);
    assert!(is_device_key(&kept.public()));
    let pushed = encrypt(&made.public(), &made.auth(), b"hello", 100).unwrap();
    assert_eq!(kept.open(&pushed).as_deref(), Some(&b"hello"[..]));
    assert_eq!(DeviceKeys::generate().open(&pushed), None);
    assert_eq!(example.open(&pushed), None);
}
