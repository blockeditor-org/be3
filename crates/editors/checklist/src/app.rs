use block_client::blocks::checklist::Checklist as ChecklistBlock;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Checklist;

pub struct ChecklistApp;

impl block_editor_plugin::BeuiApp for ChecklistApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Checklist editor=editor />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation
            .client()
            .create_block(ChecklistBlock::default())
            .id())
    }
}
