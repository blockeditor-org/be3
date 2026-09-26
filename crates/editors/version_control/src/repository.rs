use block_editor_beui::be_block::RepositoryContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct RepositoryApp;

impl block_editor_beui::BeuiApp for RepositoryApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <RepositoryView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&RepositoryContent::default()))
    }
}
