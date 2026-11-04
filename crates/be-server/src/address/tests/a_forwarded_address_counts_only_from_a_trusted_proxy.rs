use super::*;

#[test]
fn a_forwarded_address_counts_only_from_a_trusted_proxy() {
    let loopback: IpAddr = "127.0.0.1".parse().unwrap();
    let proxy: IpAddr = "10.0.0.2".parse().unwrap();
    let stranger: IpAddr = "203.0.113.9".parse().unwrap();
    let trusted = [loopback, proxy];
    assert_eq!(
        client_address(stranger, Some("198.51.100.1"), &trusted),
        stranger,
        "a peer that is no proxy cannot name another address"
    );
    assert_eq!(
        client_address(loopback, Some("198.51.100.1"), &trusted),
        "198.51.100.1".parse::<IpAddr>().unwrap()
    );
    assert_eq!(
        client_address(
            loopback,
            Some("198.51.100.7, 198.51.100.1, 10.0.0.2"),
            &trusted
        ),
        "198.51.100.1".parse::<IpAddr>().unwrap(),
        "what the client itself wrote before the first untrusted hop is ignored"
    );
    assert_eq!(
        client_address(loopback, Some("not an address"), &trusted),
        loopback
    );
    assert_eq!(client_address(loopback, None, &trusted), loopback);
    assert_eq!(
        client_address(
            "::ffff:127.0.0.1".parse().unwrap(),
            Some("[2001:db8::1]:443"),
            &trusted
        ),
        "2001:db8::1".parse::<IpAddr>().unwrap(),
        "a mapped IPv4 peer is its IPv4 address, and a hop may carry a port"
    );
}
