use crate::metadata::{BlockMetadata, ICON, MATERIAL_ICON};

#[test]
fn an_icon_saved_as_a_material_codepoint_reads_as_its_lucide_name() {
    let mut saved = BlockMetadata::named("Notes");
    saved.insert(&MATERIAL_ICON, &'\u{f09a}');
    let decoded = BlockMetadata::decode(&saved.encode()).unwrap();
    assert_eq!(decoded.icon().as_deref(), Some("star"));

    let mut unknown = BlockMetadata::named("Notes");
    unknown.insert(&MATERIAL_ICON, &'\u{e000}');
    assert_eq!(unknown.icon(), None);

    let mut both = decoded;
    both.insert(&ICON, &"folder".to_owned());
    assert_eq!(both.icon().as_deref(), Some("folder"));
}
