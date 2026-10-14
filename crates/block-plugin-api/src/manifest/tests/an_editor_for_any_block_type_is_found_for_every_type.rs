use super::*;

#[test]
fn an_editor_for_any_block_type_is_found_for_every_type() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    document.editors[0].block_type = crate::ANY_BLOCK_TYPE_NAME.into();
    let manifest = document.into_manifest().expect("any is a block type");
    assert_eq!(manifest.editors[0].block_type, crate::ANY_BLOCK_TYPE);
    let found = manifest
        .editor([9; 16])
        .expect("any block type has this editor");
    assert_eq!(found.display_name, "Counter");
}
