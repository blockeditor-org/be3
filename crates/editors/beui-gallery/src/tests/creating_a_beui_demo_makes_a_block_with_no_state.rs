use super::*;

#[test]
fn creating_a_beui_demo_makes_a_block_with_no_state() {
    let host = EditorHost::default();
    let creation = Creation::new(host.clone());

    let created = <BeuiGalleryApp as block_editor_beui::BeuiApp>::create_block(&creation)
        .expect("a beui demo can always be made");

    let commands = host.take_graph_commands();
    let [GraphCommand::Create { id, block_type, .. }] = commands.as_slice() else {
        panic!(
            "expected one block to be created, got {} commands",
            commands.len()
        );
    };
    assert_eq!(*id, created);
    assert_eq!(*block_type, BeuiDemoContent::CONTENT_TYPE);
}
