use block_editor_plugin::be_block::RepositoryContent;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct RepositoryApp;

impl block_editor_plugin::BeuiApp for RepositoryApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <RepositoryView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&RepositoryContent::default()))
    }
}
