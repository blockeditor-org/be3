use super::*;

#[test]
fn manifest_from_json_reads_a_document() {
    let manifest = manifest_from_json(DOCUMENT).expect("the document is valid");
    assert_eq!(manifest.identity.id, "be3.counter");
    assert_eq!(manifest.identity.name, "Counter");
    assert_eq!(manifest.identity.version, "0.1.0");
    assert_eq!(manifest.editors.len(), 1);
    let editor = &manifest.editors[0];
    assert_eq!(
        editor.block_type,
        Uuid::parse_str("636f756e-7465-722d-626c-6f636b2d0001")
            .expect("a uuid")
            .into_bytes()
    );
    assert_eq!(editor.icon, "\u{eb8d}");
    assert_eq!(editor.regions, vec![EditorRegion::Frame]);
    assert_eq!(manifest.entry_point, "counter.wasm");
}
