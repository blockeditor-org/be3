use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_example_scoreboard::ScoreboardContent;
use uuid::Uuid;

mod ui;

use ui::Scoreboard;

pub struct ScoreboardRoot;

#[component]
impl be_sdk_beui::BeuiRoot for ScoreboardRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <Scoreboard editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&ScoreboardContent::default()))
    }
}
