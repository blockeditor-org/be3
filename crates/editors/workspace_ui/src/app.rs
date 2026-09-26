mod access;
mod artifact;
mod block_data;
mod linked;
mod menu;
mod panel;
mod status;
mod tab;
pub(crate) mod workspace;

use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;

use workspace::WorkspaceShell;

pub struct WorkspaceUiApp;

impl block_editor_beui::BeuiApp for WorkspaceUiApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <WorkspaceShell editor={editor} />
        }
    }
}
