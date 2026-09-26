use super::*;
use block_editor_plugin::GraphCommand;
use block_editor_plugin::be_block::BlockContent;

#[test]
fn a_new_view_starts_as_a_spreadsheet() {
    let host = EditorHost::default();
    let creation = Creation::new(host.clone());

    let id = DatabaseViewApp::create_block(&creation).unwrap();

    let created: Vec<(Uuid, Option<Vec<u8>>)> = host
        .take_graph_commands()
        .into_iter()
        .filter_map(|command| match command {
            GraphCommand::Create { id, content, .. } => Some((id, content)),
            _ => None,
        })
        .collect();
    let view = created
        .iter()
        .find_map(|(block, content)| (*block == id).then_some(content.as_ref()).flatten())
        .expect("the view was given content");
    let view = DatabaseViewContent::decode(view).unwrap().root();
    assert_eq!(view.kind, DatabaseViewKind::Spreadsheet);
    let database = view.database;
    assert!(created.iter().any(|(block, _)| Some(*block) == database));
}
