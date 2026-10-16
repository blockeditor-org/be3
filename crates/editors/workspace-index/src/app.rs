use block_editor_beui::be_block::{BlockContent, FolderContent, WorkspaceContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{BlockQuery, Loaded};
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod drop;
mod entries;
mod ui;

use ui::FolderEditor;

pub struct WorkspaceIndexApp;

impl block_editor_beui::BeuiApp for WorkspaceIndexApp {
    fn view(editor: Editor) -> NodeId {
        let references = [BlockQuery::References(editor.block_id())];
        let loaded = match editor.host().block_type() == Some(WorkspaceContent::CONTENT_TYPE) {
            true => editor.loaded::<WorkspaceContent>(&references),
            false => editor.loaded::<FolderContent>(&references),
        };
        view! {
            <Loaded loaded={loaded}>
                <FolderEditor editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&FolderContent::default()))
    }
}
