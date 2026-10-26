use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use counter_types::CounterContent;
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterApp;

#[component]
impl block_editor_beui::BeuiApp for CounterApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <Counter editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}
