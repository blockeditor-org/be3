use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_checklist::ChecklistContent;
use uuid::Uuid;

mod ui;

use ui::Checklist;

pub struct ChecklistRoot;

/// The room the checklist asks for where it is placed (a card on a canvas): its header, the
/// card to add and filter tasks, and a few rows of the task list below them, which otherwise
/// gets only what the two above leave it.
const INTRINSIC_SIZE: Vec2 = Vec2::new(440.0, 480.0);

#[component]
impl be_sdk_beui::BeuiRoot for ChecklistRoot {
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

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}
