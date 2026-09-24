use std::sync::Arc;

use block::Block;
use block_client::BlockClient;
use block_client::blocks::counter::Counter;
use block_client::blocks::workspace_index::{WorkspaceIndex, WorkspaceIndexOperation};
use block_editor_plugin::{Drag, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::WorkspaceIndexApp;

mod an_empty_folder_says_so;
mod dropping_a_block_reports_whether_the_folder_takes_it;
mod every_entry_in_the_index_gets_a_cell;

struct Fixture {
    editor: BeuiTest<WorkspaceIndexApp>,
    host: EditorHost,
}

fn editor(entries: usize) -> (Fixture, Vec<Uuid>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let folder = client.create_block(WorkspaceIndex::default());
    let mut children = Vec::new();
    for _ in 0..entries {
        let child = client.create_block(Counter::default());
        folder.operate(WorkspaceIndexOperation::Add(child.id()));
        children.push(child.id());
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, folder.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (Fixture { editor, host }, children)
}
