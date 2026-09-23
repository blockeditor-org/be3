use super::*;
use block_editor_plugin::be_block::BlockContent;

#[test]
fn a_new_view_starts_as_a_spreadsheet() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let host = EditorHost::default();
    let creation = Creation::new(host.clone(), Arc::clone(&client));

    let id = DatabaseViewApp::create_block(&creation).unwrap();

    let seeded = host.take_seeded_content();
    let view = seeded
        .iter()
        .find(|seeded| seeded.block == id)
        .expect("the view was given content");
    let view = DatabaseViewContent::decode(&view.bytes).unwrap().root();
    assert_eq!(view.kind, DatabaseViewKind::Spreadsheet);
    let database = view.database.and_then(|database| database.as_direct());
    assert!(seeded.iter().any(|seeded| Some(seeded.block) == database));
}
