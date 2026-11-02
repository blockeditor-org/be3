use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_checklist::ChecklistContent;
use uuid::Uuid;

mod ui;

use ui::Checklist;

pub struct ChecklistRoot;

#[component]
impl be_sdk_beui::BeuiApp for ChecklistRoot {
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
