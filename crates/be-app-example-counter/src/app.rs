use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_example_counter::CounterContent;
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterApp;

#[component]
impl be_sdk_beui::BeuiApp for CounterApp {
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
