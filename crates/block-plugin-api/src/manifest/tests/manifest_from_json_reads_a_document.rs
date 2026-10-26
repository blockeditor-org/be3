use super::*;

#[test]
fn manifest_from_json_reads_a_document() {
    let manifest = manifest_from_json(DOCUMENT).expect("the document is valid");
    assert_eq!(manifest.identity.id, "be3.counter");
    assert_eq!(manifest.identity.name, "Counter");
    assert_eq!(manifest.identity.version, "0.1.0");
    assert_eq!(manifest.editors.len(), 1);
    let editor = &manifest.editors[0];
    assert_eq!(editor.name, "counter");
    assert_eq!(
        editor.id,
        Uuid::parse_str("5b1f3c1e-2f7a-4d0e-9a51-0c6f2b8e7d41")
            .expect("a uuid")
            .into_bytes()
    );
    assert_eq!(
        editor.edits,
        Edits::Type(
            Uuid::parse_str("636f756e-7465-722d-626c-6f636b2d0001")
                .expect("a uuid")
                .into_bytes()
        )
    );
    assert_eq!(editor.icon, "\u{e0ef}");
    assert_eq!(editor.regions, vec![EditorRegion::Frame]);
    assert_eq!(manifest.entry_point, "counter.wasm");
}
