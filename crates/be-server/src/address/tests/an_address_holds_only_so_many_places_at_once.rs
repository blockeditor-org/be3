use super::*;

#[test]
fn an_address_holds_only_so_many_places_at_once() {
    let places = PerAddress::new(2);
    let first = places.take("2001:db8::1".parse().unwrap()).unwrap();
    let _second = places.take("2001:db8::2".parse().unwrap()).unwrap();
    assert!(
        places.take("2001:db8::ffff:3".parse().unwrap()).is_none(),
        "an IPv6 /64 counts as one address"
    );
    assert!(places.take("2001:db8:0:1::1".parse().unwrap()).is_some());
    assert!(places.take("192.0.2.1".parse().unwrap()).is_some());
    drop(first);
    assert!(
        places.take("2001:db8::3".parse().unwrap()).is_some(),
        "a place is given back when dropped"
    );
}
