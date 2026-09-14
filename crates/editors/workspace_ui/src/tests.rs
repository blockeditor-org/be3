use std::sync::Arc;

use block::Block;
use block_client::BlockClient;
use block_client::blocks::file_tree::FileTree;
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::WorkspaceUiApp;

mod a_block_opened_from_a_tab_replaces_it;
mod a_shown_block_is_reported_as_focused;
mod a_tab_walks_back_and_forward_through_its_history;
mod closing_the_only_tab_leaves_the_blank_workspace;

fn editor() -> (EditorTest<'static, WorkspaceUiApp>, EditorHost, Uuid) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let workspace = client.create_block(block_client::blocks::workspace_ui::WorkspaceUi::new());
    let opened = client.create_block(FileTree::new());
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let mut app = WorkspaceUiApp::default();
    app.connect(host.clone(), client, workspace.id());
    let mut editor = EditorTest::new(app);
    editor.step();
    (editor, host, opened.id())
}
