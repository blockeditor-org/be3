mod artifact;
mod block_data;
mod chrome;
mod linked;
mod menu;
mod panel;
mod status;
mod tab;
pub(crate) mod workspace;

use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

use workspace::WorkspaceShell;

pub struct WorkspaceUiApp;

impl block_editor_plugin::BeuiApp for WorkspaceUiApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <WorkspaceShell editor={editor} />
        }
    }
}
