use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use terminal_types::TerminalContent;
use uuid::Uuid;

mod offline;
mod session;
mod ui;

pub use offline::setup_command;
use ui::TerminalBlock;

pub struct TerminalApp;

#[component]
impl block_editor_beui::BeuiApp for TerminalApp {
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
