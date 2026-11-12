use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_chat::ChatContent;
use uuid::Uuid;

pub(crate) mod ui;

use ui::Chat;

pub struct ChatRoot;

#[component]
impl be_sdk_beui::BeuiRoot for ChatRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded label="Loading the chat">
                    <Chat editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&ChatContent::default()))
    }
}
