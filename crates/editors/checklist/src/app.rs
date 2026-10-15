use block_editor_beui::Loaded;
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
        let loaded = editor.loaded::<block_editor_beui::be_block::ChecklistContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <Checklist editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&ChecklistContent::default()))
    }
}
