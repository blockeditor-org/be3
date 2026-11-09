use super::*;
use base64::Engine;

/// RFC 8291's Appendix A: the example's shared secret, keys and salt give its exact message,
/// and it opens again to the plaintext.
#[test]
fn web_push_encryption_matches_rfc_8291() {
    let decode = |text: &str| {
        let text: String = text.split_whitespace().collect();
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(text)
            .unwrap()
    };
    let plain = b"When I grow up, I want to be a watermelon";
    let as_public = decode(
        "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIg
         Dll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8",
    );
    let ua_public = decode(
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcx
         aOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
    );
    let salt: [u8; 16] = decode("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
    let auth = decode("BTBZMqHH6r4Tts7J_aSIgg");
    let secret = decode("kyrL1jIIOHEzg3sM2ZWRHDRB62YACZhhSlknJ672kSs");
    let expected = decode(
        "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27ml
         mlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPT
         pK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN",
    );

    let sealed = push::seal(&secret, &as_public, &ua_public, &auth, &salt, plain);
    assert_eq!(sealed, expected);
    assert_eq!(push::sender_key(&sealed), Some(&as_public[..]));
    assert_eq!(
        push::open(&secret, &ua_public, &auth, &sealed).as_deref(),
        Some(&plain[..])
    );
    let mut tampered = sealed.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert_eq!(push::open(&secret, &ua_public, &auth, &tampered), None);
}
