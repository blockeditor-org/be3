use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_terminal::TerminalContent;
use uuid::Uuid;

mod offline;
mod session;
mod ui;

pub use offline::setup_command;
use ui::TerminalBlock;

pub struct TerminalApp;

#[component]
impl be_sdk_beui::BeuiApp for TerminalApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded label="Loading the terminal">
                    <TerminalBlock editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&TerminalContent::default()))
    }
}
