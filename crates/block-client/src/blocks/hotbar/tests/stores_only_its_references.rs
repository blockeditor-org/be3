use super::*;

#[test]
fn stores_only_its_references() {
    let mut block = Hotbar::new();
    assert!(block.references().is_empty());
    assert_eq!(
        serde_json::to_string(&block).unwrap(),
        "{\"references\":[]}"
    );

    let linked = Uuid::new_v4();
    let operation = Hotbar::bridged_references(vec![linked]).expect("references are bridged");
    Hotbar::apply_operation(&mut block, &operation);
    assert_eq!(block.references(), [linked]);
}
