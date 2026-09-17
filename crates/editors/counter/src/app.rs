use block_client::blocks::counter::Counter as CounterBlock;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterApp;

impl block_editor_plugin::BeuiApp for CounterApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Counter editor=editor />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(CounterBlock::default()).id())
    }
}
