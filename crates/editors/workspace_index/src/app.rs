use block_client::blocks::workspace_index::WorkspaceIndex;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod drop;
mod entries;
mod ui;

use ui::FolderEditor;

pub struct WorkspaceIndexApp;

impl block_editor_plugin::BeuiApp for WorkspaceIndexApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <FolderEditor editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(WorkspaceIndex).id())
    }
}
