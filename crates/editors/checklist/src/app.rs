use block_editor_beui::be_block::ChecklistContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Checklist;

pub struct ChecklistApp;

impl block_editor_beui::BeuiApp for ChecklistApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Checklist editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&ChecklistContent::default()))
    }
}
