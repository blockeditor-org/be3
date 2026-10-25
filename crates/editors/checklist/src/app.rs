use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use checklist_types::ChecklistContent;
use uuid::Uuid;

mod ui;

use ui::Checklist;

pub struct ChecklistApp;

#[component]
impl block_editor_beui::BeuiApp for ChecklistApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <Checklist editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&ChecklistContent::default()))
    }
}
