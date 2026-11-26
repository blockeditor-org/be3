use super::*;

#[test]
fn the_same_value_is_always_the_same_bytes() {
    let forwards: HashMap<String, u32> = (0..50).map(|i| (i.to_string(), i)).collect();
    let backwards: HashMap<String, u32> = (0..50).rev().map(|i| (i.to_string(), i)).collect();
    assert_eq!(bytes(&forwards), bytes(&backwards));
    // Whatever holds the map: entries are sorted by their key's bytes, not by Ord.
    let ordered: BTreeMap<String, u32> = forwards.clone().into_iter().collect();
    assert_eq!(bytes(&ordered), bytes(&forwards));
    let set: HashSet<u16> = (0..20).collect();
    let ordered_set: BTreeSet<u16> = (0..20).rev().collect();
    assert_eq!(bytes(&set), bytes(&ordered_set));

    let quiet = f64::from_bits(0x7ff8_0000_0000_0001);
    let other = f64::from_bits(0xfff0_0000_0000_0002);
    assert!(quiet.is_nan() && other.is_nan());
    assert_eq!(bytes(&quiet), bytes(&other));
    assert_ne!(bytes(&-0.0f64), bytes(&0.0f64));
}
