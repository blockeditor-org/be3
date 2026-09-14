use super::*;

#[test]
fn serialization_round_trip() {
    let block = PanZoom::default();
    let encoded = serde_json::to_vec(&block).unwrap();
    assert_eq!(serde_json::from_slice::<PanZoom>(&encoded).unwrap(), block);
}
