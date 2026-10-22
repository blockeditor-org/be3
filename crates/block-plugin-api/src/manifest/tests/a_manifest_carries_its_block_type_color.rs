use super::*;

#[test]
fn a_manifest_carries_its_block_type_color() {
    let source = DOCUMENT.replace(
        "\"display_name\": \"Counter\",",
        "\"display_name\": \"Counter\", \"color\": [65, 145, 210],",
    );
    let manifest = manifest_from_json(&source).unwrap();
    assert_eq!(manifest.editors[0].color, Some([65, 145, 210]));
    assert!(ManifestDocument::parse(&source.replace("65, 145, 210", "256, 145, 210")).is_err());
}
