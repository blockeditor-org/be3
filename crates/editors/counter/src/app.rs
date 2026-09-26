use block_editor_beui::be_block::CounterContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterApp;

impl block_editor_beui::BeuiApp for CounterApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Counter editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}
