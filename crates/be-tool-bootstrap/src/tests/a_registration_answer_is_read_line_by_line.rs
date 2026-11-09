use super::*;

#[test]
fn a_registration_answer_is_read_line_by_line() {
    let answer = "ip=10.100.0.23\nhub_pubkey=abc=\nendpoint=[2001:db8::1]:51822\n";
    assert_eq!(
        nativelink::parse_registration(answer),
        Some(nativelink::Registration {
            ip: "10.100.0.23".to_owned(),
            hub_key: "abc=".to_owned(),
            endpoint: "[2001:db8::1]:51822".to_owned(),
        })
    );
    assert_eq!(nativelink::parse_registration("ip=10.100.0.23\n"), None);
    assert_eq!(nativelink::parse_registration(""), None);
}
