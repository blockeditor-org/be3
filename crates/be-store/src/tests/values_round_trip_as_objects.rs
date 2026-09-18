use super::*;

#[test]
fn values_round_trip_as_objects() {
    let vault = vault(7);
    let manifest = vault.write(CONTENT, &pseudorandom(9000, 31)).unwrap();
    let hash = vault.put_value(&manifest).unwrap();
    assert!(vault.has_value(hash).unwrap());
    assert_eq!(vault.get_value::<Manifest>(hash).unwrap(), manifest);
    assert_eq!(vault.put_value(&manifest).unwrap(), hash);
    assert_eq!(Hash::from_hex(&hash.to_hex()), Some(hash));
}
