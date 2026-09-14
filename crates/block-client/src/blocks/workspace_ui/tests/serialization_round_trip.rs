use super::*;

#[test]
fn serialization_round_trip() {
    let workspace = WorkspaceUi::new();
    let encoded = serde_json::to_vec(&workspace).unwrap();
    assert_eq!(
        serde_json::from_slice::<WorkspaceUi>(&encoded).unwrap(),
        workspace
    );
}
