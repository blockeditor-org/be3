use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;
use workspace_ui_types::FolderContent;

mod drop;
mod entries;
mod ui;

use ui::FolderEditor;

pub struct WorkspaceIndexApp;

#[component]
impl block_editor_beui::BeuiApp for WorkspaceIndexApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <FolderEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&FolderContent::default()))
    }
}
