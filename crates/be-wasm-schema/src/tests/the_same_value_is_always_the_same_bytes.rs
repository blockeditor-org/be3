use super::*;

#[test]
fn the_same_value_is_always_the_same_bytes() {
    let forwards: HashMap<String, u32> = (0..50).map(|i| (i.to_string(), i)).collect();
    let backwards: HashMap<String, u32> = (0..50).rev().map(|i| (i.to_string(), i)).collect();
    assert_eq!(encode(&forwards), encode(&backwards));

    let quiet = f64::from_bits(0x7ff8_0000_0000_0001);
    let other = f64::from_bits(0xfff0_0000_0000_0002);
    assert!(quiet.is_nan() && other.is_nan());
    assert_eq!(encode(&quiet), encode(&other));
    assert_ne!(encode(&-0.0f64), encode(&0.0f64));
}
