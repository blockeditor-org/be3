use super::*;

#[test]
fn manifest_document_rejects_a_bad_block_type() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    document.editors.0[0].1.edits = Some("not a uuid".into());
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::InvalidBlockType)
    );
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    document.editors.0[0].1.id = "not a uuid".into();
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::InvalidEditorId)
    );
}
