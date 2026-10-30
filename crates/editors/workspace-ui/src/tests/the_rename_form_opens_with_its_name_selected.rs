use super::*;

#[test]
fn the_rename_form_opens_with_its_name_selected() {
    let (workspace, notes) = editor();
    let mut info = block_editor_beui::BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Notes");
    workspace.test.store().add_block(info);
    let mut fixture = renaming(&workspace, notes);
    fixture.settle();
    fixture.test.type_text("Journal");
    fixture.settle();
    assert_eq!(
        fixture.test.take_renames().last(),
        Some(&(notes, Some("Journal".to_owned()))),
        "what is typed first replaces the whole name"
    );
}
