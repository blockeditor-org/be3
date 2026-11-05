use super::*;

/// Clients that are not signed in are held to their own share: one address (an IPv6 /48 however
/// many /64s it uses) to `per_address` of them, and all of them to `limit`, past which a newcomer
/// lets go of the oldest rather than being turned away.
#[tokio::test]
async fn clients_not_signed_in_never_fill_the_server() {
    let unsigned = Unsigned::new(4, 2);
    let admit = |address: IpAddr| {
        let eviction = Arc::new(Eviction::default());
        unsigned
            .admit(address, &eviction)
            .map(|anonymous| (anonymous, eviction))
    };

    let site: u128 = 0x2001_0db8_0001 << 80;
    let first = admit(IpAddr::from(std::net::Ipv6Addr::from(site | 1))).unwrap();
    let _second = admit(IpAddr::from(std::net::Ipv6Addr::from(site | (7 << 64)))).unwrap();
    assert!(
        admit(IpAddr::from(std::net::Ipv6Addr::from(site | (9 << 64) | 3))).is_none(),
        "another /64 of the same /48 took a third place"
    );

    let mut others = Vec::new();
    for last in 1..=2 {
        others.push(admit(IpAddr::from([192, 0, 2, last])).unwrap());
    }
    assert_eq!(unsigned.len(), 4);
    assert!(!first.1.evicted());
    let newest = admit(IpAddr::from([198, 51, 100, 1])).expect("a newcomer was turned away");
    assert_eq!(unsigned.len(), 4);
    assert!(first.1.evicted(), "the oldest was not let go of");
    first.1.wait().await;
    assert!(!newest.1.evicted());

    drop(first);
    assert_eq!(
        unsigned.len(),
        4,
        "dropping an evicted place freed another's"
    );
    drop(newest);
    assert_eq!(
        unsigned.len(),
        3,
        "a place signed in or ended was not given back"
    );
    assert!(
        admit(IpAddr::from(std::net::Ipv6Addr::from(site | 5))).is_some(),
        "the /48 did not get its evicted place back"
    );
}
