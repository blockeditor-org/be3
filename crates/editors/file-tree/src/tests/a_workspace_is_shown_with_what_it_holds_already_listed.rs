use block_editor_beui::be_block::BlockContent;
use block_editor_beui::be_block::EditorView;
use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn a_workspace_is_shown_with_what_it_holds_already_listed() {
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_view_block(Some(tree));
    let mut test = BeuiTest::<FileTreeApp>::new(OwnedEditor::new(host.clone(), tree));
    let note = |parent: Uuid| {
        BlockInfo::new(
            Uuid::new_v4(),
            Uuid::from_u128(0x6e6f_7465),
            BlockParent::Block(parent),
        )
    };
    let mine = note(MY_FILES);
    let team = Uuid::new_v4();
    let theirs = note(team);
    let mut workspace = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    workspace.author = Some(host.account_id());
    workspace.set_name("Team");
    workspace.references = vec![theirs.id];
    test.store()
        .add_block(files_workspace(&host, std::slice::from_ref(&mine)));
    test.store().add_block(workspace);
    test.store().add_block(mine.clone());
    test.store().add_block(theirs.clone());
    test.hold(None, EditorView::document(None));

    let rows = [MY_FILES, team, mine.id, theirs.id].map(|id| format!("file-tree.{id}.row"));
    for _ in 0..20 {
        test.step(Vec::new());
        let shown = rows.iter().filter(|row| test.shown(row)).count();
        assert!(
            shown == 0 || shown == rows.len(),
            "the files are shown only once every workspace is listed"
        );
    }
    assert!(rows.iter().all(|row| test.shown(row)));
}
