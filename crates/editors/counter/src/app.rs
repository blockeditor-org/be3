use block_editor_beui::Loaded;
use block_editor_beui::be_block::CounterContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterApp;

impl block_editor_beui::BeuiApp for CounterApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <Counter editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}
