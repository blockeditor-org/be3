use block_editor_beui::Loaded;
use terminal_types::TerminalContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod offline;
mod session;
mod ui;

pub use offline::setup_command;
use ui::TerminalBlock;

pub struct TerminalApp;

impl block_editor_beui::BeuiApp for TerminalApp {
    fn view(editor: Editor) -> Root {
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
