use super::*;

#[test]
fn an_editor_that_edits_nothing_is_never_offered() {
    let block_type = Uuid::from_u128(1);
    let entry = |editor: u128, edits: Edits| EditorEntry {
        editor: Uuid::from_u128(editor),
        edits,
        name: format!("editor {editor}"),
        icon: None,
        extensions: Vec::new(),
    };
    let catalog = BlockCatalog::default().with_editors([
        entry(10, Edits::Type(block_type.into_bytes())),
        entry(11, Edits::Nothing),
        entry(12, Edits::Any),
    ]);
    let offered: Vec<Uuid> = catalog
        .editors_for(block_type, None)
        .into_iter()
        .map(|entry| entry.editor)
        .collect();
    assert_eq!(offered, vec![Uuid::from_u128(10), Uuid::from_u128(12)]);
    assert!(!catalog.edits(Uuid::from_u128(11), block_type, None));
    assert_eq!(
        catalog.own_editor(block_type, None),
        Some(Uuid::from_u128(10))
    );
}
