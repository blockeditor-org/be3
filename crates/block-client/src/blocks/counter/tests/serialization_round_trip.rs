use super::*;

#[test]
fn serialization_round_trip() {
    let counter = Counter::new();
    let encoded = serde_json::to_vec(&counter).unwrap();
    assert_eq!(
        serde_json::from_slice::<Counter>(&encoded).unwrap(),
        counter
    );
}
