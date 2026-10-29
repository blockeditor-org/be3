use super::*;

#[test]
fn two_editors_may_edit_one_block_type() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let mut outline = document.editors.0[0].1.clone();
    outline.id = "0d3e6f0a-7c55-4a8e-b2e4-5f19d3c2a7b6".into();
    outline.display_name = "Counter outline".into();
    document.editors.0.push(("outline".into(), outline.clone()));
    let manifest = document
        .clone()
        .into_manifest()
        .expect("two editors of one type");
    assert_eq!(manifest.editors[0].edits, manifest.editors[1].edits);
    assert_ne!(manifest.editors[0].id, manifest.editors[1].id);

    document.editors.0[1].1.id = document.editors.0[0].1.id.clone();
    assert_eq!(
        document.clone().into_manifest(),
        Err(ManifestError::DuplicateEditor)
    );
    document.editors.0[1] = ("counter".into(), outline);
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::DuplicateEditor)
    );
}
