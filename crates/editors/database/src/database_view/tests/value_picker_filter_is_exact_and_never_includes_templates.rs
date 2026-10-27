use block_editor_beui::PickAnchor;

use super::*;

#[test]
fn value_picker_filter_is_exact_and_never_includes_templates() {
    let block_type = Uuid::new_v4();
    let filtered = value_block_filter(
        "Related",
        DatabaseBlockPickRequest {
            field_id: Uuid::new_v4(),
            block_type: Some(block_type),
            anchor: PickAnchor::Centre,
        },
    );
    assert_eq!(filtered.name, "Related");
    assert_eq!(filtered.block_types, vec![block_type.into_bytes()]);
    assert!(!filtered.templates);

    let unrestricted = value_block_filter(
        "Related",
        DatabaseBlockPickRequest {
            field_id: Uuid::new_v4(),
            block_type: None,
            anchor: PickAnchor::Centre,
        },
    );
    assert!(unrestricted.block_types.is_empty());
}
